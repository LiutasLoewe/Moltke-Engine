extends RefCounted
class_name ProvincePathfinder

## Dijkstra pathfinding over WorldMapData adjacency with stable tie-breaking.
## Edge cost is the positive integer cost of entering the destination terrain.


static func find_path(
	world_map: WorldMapData,
	start_id: String,
	goal_id: String,
	options: Dictionary = {}
) -> Dictionary:
	if world_map == null:
		return {"found": false, "path": [], "cost": 0, "error": "World map is required."}
	if world_map.get_province(start_id) == null or world_map.get_province(goal_id) == null:
		return {"found": false, "path": [], "cost": 0, "error": "Start or goal province does not exist."}
	if start_id == goal_id:
		return {"found": true, "path": [start_id], "cost": 0, "error": ""}
	if not options.get("terrain_costs", {}) is Dictionary:
		return {"found": false, "path": [], "cost": 0, "error": "terrain_costs must be an object."}
	if not options.get("blocked_province_ids", []) is Array:
		return {"found": false, "path": [], "cost": 0, "error": "blocked_province_ids must be an array."}

	var terrain_costs: Dictionary = options.get("terrain_costs", {})
	var blocked_ids: Array = options.get("blocked_province_ids", [])
	if blocked_ids.has(start_id) or blocked_ids.has(goal_id):
		return {"found": false, "path": [], "cost": 0, "error": "Start or goal is blocked."}
	for terrain_type in terrain_costs:
		var raw_cost: Variant = terrain_costs[terrain_type]
		if typeof(raw_cost) != TYPE_INT or int(raw_cost) < 1:
			return {
				"found": false,
				"path": [],
				"cost": 0,
				"error": "Terrain costs must be positive integers."
			}

	var maximum_cost := int(options.get("maximum_cost", -1))
	var distances: Dictionary = {start_id: 0}
	var previous: Dictionary = {}
	var heap: Array[Dictionary] = []
	_heap_push(heap, {"id": start_id, "cost": 0})

	while not heap.is_empty():
		var current: Dictionary = _heap_pop(heap)
		var current_id := str(current["id"])
		var current_cost := int(current["cost"])
		if current_cost != int(distances.get(current_id, -1)):
			continue
		if current_id == goal_id:
			break

		var adjacent_provinces := world_map.get_adjacent_provinces(current_id)
		adjacent_provinces.sort_custom(func(left: ProvinceData, right: ProvinceData) -> bool:
			return left.id < right.id
		)
		for adjacent in adjacent_provinces:
			if blocked_ids.has(adjacent.id):
				continue
			var edge_cost := int(terrain_costs.get(adjacent.terrain_type, 1))
			var candidate_cost := current_cost + edge_cost
			if maximum_cost >= 0 and candidate_cost > maximum_cost:
				continue

			var known_cost := int(distances.get(adjacent.id, 2147483647))
			var known_predecessor := str(previous.get(adjacent.id, "~"))
			if candidate_cost < known_cost or (
				candidate_cost == known_cost and current_id < known_predecessor
			):
				distances[adjacent.id] = candidate_cost
				previous[adjacent.id] = current_id
				_heap_push(heap, {"id": adjacent.id, "cost": candidate_cost})

	if not distances.has(goal_id):
		return {"found": false, "path": [], "cost": 0, "error": "No route exists."}

	var reverse_path: Array[String] = [goal_id]
	var cursor := goal_id
	while cursor != start_id:
		if not previous.has(cursor):
			return {"found": false, "path": [], "cost": 0, "error": "Could not reconstruct route."}
		cursor = str(previous[cursor])
		reverse_path.append(cursor)
	reverse_path.reverse()
	return {"found": true, "path": reverse_path, "cost": int(distances[goal_id]), "error": ""}


static func _heap_push(heap: Array[Dictionary], item: Dictionary) -> void:
	heap.append(item)
	var index := heap.size() - 1
	while index > 0:
		var parent := (index - 1) / 2
		if not _has_higher_priority(heap[index], heap[parent]):
			break
		var temporary: Dictionary = heap[parent]
		heap[parent] = heap[index]
		heap[index] = temporary
		index = parent


static func _heap_pop(heap: Array[Dictionary]) -> Dictionary:
	var root: Dictionary = heap[0]
	var last: Dictionary = heap.pop_back()
	if heap.is_empty():
		return root
	heap[0] = last

	var index := 0
	while true:
		var left := index * 2 + 1
		var right := left + 1
		var best := index
		if left < heap.size() and _has_higher_priority(heap[left], heap[best]):
			best = left
		if right < heap.size() and _has_higher_priority(heap[right], heap[best]):
			best = right
		if best == index:
			break
		var temporary: Dictionary = heap[index]
		heap[index] = heap[best]
		heap[best] = temporary
		index = best
	return root


static func _has_higher_priority(left: Dictionary, right: Dictionary) -> bool:
	var left_cost := int(left["cost"])
	var right_cost := int(right["cost"])
	if left_cost != right_cost:
		return left_cost < right_cost
	return str(left["id"]) < str(right["id"])
