extends Node
class_name SimulationRuntime

## Coordinates the fixed-step clock, trigger evaluation, and game-state snapshot.
signal event_triggered(event_payload: Dictionary)

const ClockScript = preload("res://engine/core/simulation_clock.gd")
const TriggerEventSystemScript = preload("res://engine/core/trigger_event_system.gd")

var clock = null
var trigger_events = null
var simulation_state: Dictionary = {}


func _ready() -> void:
	clock = ClockScript.new()
	add_child(clock)
	trigger_events = TriggerEventSystemScript.new()
	clock.tick_advanced.connect(_on_tick_advanced)
	trigger_events.event_triggered.connect(_on_event_triggered)


func configure(initial_state: Dictionary, event_definitions: Array) -> Array[String]:
	var errors: Array[String] = trigger_events.configure(event_definitions)
	if not errors.is_empty():
		return errors
	simulation_state = initial_state.duplicate(true)
	return errors


func get_snapshot() -> Dictionary:
	return {
		"simulation_state": simulation_state.duplicate(true),
		"clock": clock.to_state(),
		"trigger_events": trigger_events.to_state()
	}


func restore_snapshot(snapshot: Dictionary) -> bool:
	var raw_simulation_state: Variant = snapshot.get("simulation_state", null)
	var raw_clock: Variant = snapshot.get("clock", null)
	var raw_trigger_events: Variant = snapshot.get("trigger_events", null)
	if not (raw_simulation_state is Dictionary):
		return false
	if not (raw_clock is Dictionary) or not (raw_trigger_events is Dictionary):
		return false

	if not trigger_events.load_state(raw_trigger_events):
		return false
	if not clock.load_state(raw_clock):
		return false
	simulation_state = raw_simulation_state.duplicate(true)
	return true


func advance_ticks(count: int = 1) -> bool:
	return clock.advance_ticks(count)


func _on_tick_advanced(tick_index: int, _year: int, _month: int, _day: int) -> void:
	trigger_events.process_tick(tick_index, simulation_state)


func _on_event_triggered(event_payload: Dictionary) -> void:
	event_triggered.emit(event_payload)
