extends SceneTree

const WorldMapDataScript = preload("res://engine/world/world_map_data.gd")
const WORLD_DATA_PATH := "res://examples/world_map_demo/world.json"

var _failures := 0


func _initialize() -> void:
	call_deferred("_run")


func _run() -> void:
	var parsed: Variant = JSON.parse_string(FileAccess.get_file_as_string(WORLD_DATA_PATH))
	_expect(parsed is Dictionary, "Demo world JSON parses into an object.")
	if not (parsed is Dictionary):
		_finish()
		return

	var world = WorldMapDataScript.new()
	_expect(world.load_from_dictionary(parsed), "Demo world passes world-data validation.")
	_expect(world.get_region_count() == 3, "World contains three regions.")
	_expect(world.get_province_count() == 6, "World contains six provinces.")
	_expect(world.get_province("north_coast") != null, "Province lookup finds north_coast.")
	_expect(
		world.get_provinces_in_region("northern_marches").size() == 2,
		"Region lookup returns its two provinces."
	)
	_expect(
		world.get_adjacent_provinces("north_coast").size() == 2,
		"Adjacency lookup returns two neighbouring provinces."
	)

	var invalid_region_payload := {
		"regions": [],
		"provinces": [
			{
				"id": "orphan",
				"name": "Orphan Province",
				"region_id": "missing_region",
				"population": 100
			}
		]
	}
	var invalid_region_world = WorldMapDataScript.new()
	_expect(
		not invalid_region_world.load_from_dictionary(invalid_region_payload),
		"Validation rejects a province that references a missing region."
	)

	var asymmetric_payload := {
		"regions": [{"id": "test_region", "name": "Test Region"}],
		"provinces": [
			{
				"id": "a",
				"region_id": "test_region",
				"adjacent_province_ids": ["b"]
			},
			{
				"id": "b",
				"region_id": "test_region",
				"adjacent_province_ids": []
			}
		]
	}
	var asymmetric_world = WorldMapDataScript.new()
	_expect(
		not asymmetric_world.load_from_dictionary(asymmetric_payload),
		"Validation rejects one-way province adjacency."
	)

	_finish()


func _expect(condition: bool, message: String) -> void:
	if condition:
		print("PASS: %s" % message)
	else:
		_failures += 1
		push_error("FAIL: %s" % message)


func _finish() -> void:
	if _failures == 0:
		print("All Moltke world-map data tests passed.")
		quit(0)
	else:
		push_error("%d Moltke world-map data test(s) failed." % _failures)
		quit(1)
