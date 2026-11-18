extends RefCounted
class_name TriggerEventSystem

## Evaluates declarative conditions against a simulation-state Dictionary.
## Definitions are content data; this class does not execute arbitrary code.
signal event_triggered(event_payload: Dictionary)

const SUPPORTED_OPERATORS: Array[String] = [
	"eq", "ne", "lt", "lte", "gt", "gte", "contains"
]

var _definitions: Dictionary = {}
var _fired_event_ids: Dictionary = {}
var _pending_events: Array[Dictionary] = []
var _last_processed_tick: int = -1


func configure(definitions: Array) -> Array[String]:
	var errors: Array[String] = []
	var candidate_definitions: Dictionary = {}

	for raw_definition in definitions:
		if not (raw_definition is Dictionary):
			errors.append("Every event definition must be an object.")
			continue

		var definition: Dictionary = raw_definition
		var event_id := str(definition.get("id", "")).strip_edges()
		if event_id.is_empty():
			errors.append("An event definition is missing its required 'id'.")
			continue
		if candidate_definitions.has(event_id):
			errors.append("Duplicate event id: %s" % event_id)
			continue

		var conditions: Variant = definition.get("conditions", [])
		if not (conditions is Array):
			errors.append("Event %s must define conditions as an array." % event_id)
			continue

		var definition_valid := true
		for raw_condition in conditions:
			if not (raw_condition is Dictionary):
				errors.append("Event %s contains a condition that is not an object." % event_id)
				definition_valid = false
				continue

			var condition: Dictionary = raw_condition
			var path := str(condition.get("path", "")).strip_edges()
			var operator := str(condition.get("operator", "")).strip_edges()
			if path.is_empty():
				errors.append("A condition in event %s is missing its state path." % event_id)
				definition_valid = false
			if not SUPPORTED_OPERATORS.has(operator):
				errors.append("Event %s uses unsupported operator '%s'." % [event_id, operator])
				definition_valid = false
			if not condition.has("value"):
				errors.append("A condition in event %s is missing its comparison value." % event_id)
				definition_valid = false

		if definition_valid:
			candidate_definitions[event_id] = definition.duplicate(true)

	if errors.is_empty():
		_definitions = candidate_definitions
		_fired_event_ids.clear()
		_pending_events.clear()
		_last_processed_tick = -1
	return errors


func process_tick(tick_index: int, simulation_state: Dictionary) -> void:
	if tick_index < 0 or tick_index <= _last_processed_tick:
		return
	_last_processed_tick = tick_index

	var event_ids: Array = _definitions.keys()
	event_ids.sort()
	for event_id_value in event_ids:
		var event_id := str(event_id_value)
		var definition: Dictionary = _definitions[event_id]
		var once := bool(definition.get("once", true))
		if once and _fired_event_ids.has(event_id):
			continue

		var interval := maxi(1, int(definition.get("interval_ticks", 1)))
		if tick_index % interval != 0:
			continue
		if not _conditions_match(definition.get("conditions", []), simulation_state):
			continue

		_fired_event_ids[event_id] = true
		var payload := {
			"id": event_id,
			"title": str(definition.get("title", event_id)),
			"description": str(definition.get("description", "")),
			"tick": tick_index
		}
		if definition.has("choices"):
			payload["choices"] = definition["choices"].duplicate(true)
		if definition.has("data"):
			payload["data"] = definition["data"].duplicate(true)
		_pending_events.append(payload)
		event_triggered.emit(payload.duplicate(true))


func drain_pending_events() -> Array[Dictionary]:
	var result := _pending_events.duplicate(true)
	_pending_events.clear()
	return result


func to_state() -> Dictionary:
	var fired_ids: Array = _fired_event_ids.keys()
	fired_ids.sort()
	return {
		"fired_event_ids": fired_ids,
		"pending_events": _pending_events.duplicate(true),
		"last_processed_tick": _last_processed_tick
	}


func load_state(state: Dictionary) -> bool:
	var raw_fired: Variant = state.get("fired_event_ids", null)
	var raw_pending: Variant = state.get("pending_events", [])
	var raw_last_tick: Variant = state.get("last_processed_tick", -1)
	if not (raw_fired is Array) or not (raw_pending is Array):
		return false
	if typeof(raw_last_tick) != TYPE_INT or int(raw_last_tick) < -1:
		return false

	var restored_fired: Dictionary = {}
	for event_id_value in raw_fired:
		var event_id := str(event_id_value)
		if not _definitions.has(event_id):
			return false
		restored_fired[event_id] = true

	var restored_pending: Array[Dictionary] = []
	for raw_event in raw_pending:
		if not (raw_event is Dictionary):
			return false
		restored_pending.append(raw_event.duplicate(true))

	_fired_event_ids = restored_fired
	_pending_events = restored_pending
	_last_processed_tick = int(raw_last_tick)
	return true


func _conditions_match(conditions: Array, simulation_state: Dictionary) -> bool:
	for condition_value in conditions:
		var condition: Dictionary = condition_value
		var lookup := _read_state_path(simulation_state, str(condition.get("path", "")))
		if not bool(lookup.get("found", false)):
			return false
		if not _compare_values(
			lookup["value"],
			str(condition.get("operator", "")),
			condition.get("value")
		):
			return false
	return true


func _read_state_path(state: Dictionary, path: String) -> Dictionary:
	var current: Variant = state
	for part in path.split(".", false):
		if not (current is Dictionary) or not current.has(part):
			return {"found": false}
		current = current[part]
	return {"found": true, "value": current}


func _compare_values(actual: Variant, operator: String, expected: Variant) -> bool:
	match operator:
		"eq":
			return actual == expected
		"ne":
			return actual != expected
		"lt", "lte", "gt", "gte":
			var actual_type := typeof(actual)
			var expected_type := typeof(expected)
			if actual_type not in [TYPE_INT, TYPE_FLOAT]:
				return false
			if expected_type not in [TYPE_INT, TYPE_FLOAT]:
				return false
			match operator:
				"lt":
					return float(actual) < float(expected)
				"lte":
					return float(actual) <= float(expected)
				"gt":
					return float(actual) > float(expected)
				"gte":
					return float(actual) >= float(expected)
		"contains":
			if actual is Array:
				return actual.has(expected)
			if actual is String:
				return actual.contains(str(expected))
	return false
