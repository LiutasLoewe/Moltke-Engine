extends Node2D

const WorldMapDataScript = preload("res://engine/world/world_map_data.gd")
const WORLD_DATA_PATH := "res://examples/world_map_demo/world.json"
const LAYOUT_PATH := "res://examples/world_map_demo/layout.json"

const BACKGROUND_COLOR := Color(0.055, 0.067, 0.09, 1.0)
const PANEL_COLOR := Color(0.12, 0.15, 0.19, 1.0)
const BORDER_COLOR := Color(0.30, 0.36, 0.43, 1.0)
const OWNER_COLORS := {
	"north_union": Color(0.22, 0.43, 0.68, 1.0),
	"eastern_republic": Color(0.24, 0.52, 0.39, 1.0),
	"western_crown": Color(0.63, 0.40, 0.23, 1.0)
}

var _world_map: WorldMapData
var _tiles: Array = []
var _tile_size := Vector2(350, 165)
var _map_origin := Vector2(70, 150)
var _tile_gap := Vector2(14, 14)
var _selected_province_id := ""
var _error_text := ""


func _ready() -> void:
	_load_demo()
	queue_redraw()


func _load_demo() -> void:
	var world_payload := _read_json_dictionary(WORLD_DATA_PATH)
	if world_payload.is_empty():
		_error_text = "Could not read the demo world data."
		return

	_world_map = WorldMapDataScript.new()
	if not _world_map.load_from_dictionary(world_payload):
		_error_text = "The demo world data failed validation. See the Output panel."
		for message in _world_map.validation_errors:
			push_error(message)
		return

	var layout_payload := _read_json_dictionary(LAYOUT_PATH)
	if layout_payload.is_empty():
		_error_text = "Could not read the demo map layout."
		return

	_tile_size = _vector2_from_array(layout_payload.get("tile_size", null), _tile_size)
	_map_origin = _vector2_from_array(layout_payload.get("origin", null), _map_origin)
	_tile_gap = _vector2_from_array(layout_payload.get("gap", null), _tile_gap)

	var raw_tiles: Variant = layout_payload.get("tiles", [])
	if raw_tiles is Array:
		_tiles = raw_tiles


func _read_json_dictionary(path: String) -> Dictionary:
	if not FileAccess.file_exists(path):
		push_error("Missing data file: %s" % path)
		return {}

	var parsed: Variant = JSON.parse_string(FileAccess.get_file_as_string(path))
	if not (parsed is Dictionary):
		push_error("Expected a JSON object in: %s" % path)
		return {}

	return parsed


func _vector2_from_array(value: Variant, fallback: Vector2) -> Vector2:
	if not (value is Array) or value.size() < 2:
		return fallback
	return Vector2(float(value[0]), float(value[1]))


func _tile_rect(tile: Dictionary) -> Rect2:
	var grid: Variant = tile.get("grid", null)
	if not (grid is Array) or grid.size() < 2:
		return Rect2()

	var grid_offset := Vector2(float(grid[0]), float(grid[1]))
	var tile_position := _map_origin + grid_offset * (_tile_size + _tile_gap)
	return Rect2(tile_position, _tile_size)


func _input(event: InputEvent) -> void:
	if _world_map == null:
		return
	if not (event is InputEventMouseButton):
		return
	if not event.pressed or event.button_index != MOUSE_BUTTON_LEFT:
		return

	for tile_value in _tiles:
		if not (tile_value is Dictionary):
			continue
		var tile: Dictionary = tile_value
		var province_id := str(tile.get("province_id", ""))
		if _tile_rect(tile).has_point(event.position):
			if _world_map.get_province(province_id) != null:
				_selected_province_id = province_id
				queue_redraw()
				get_viewport().set_input_as_handled()
				return


func _draw() -> void:
	draw_rect(Rect2(Vector2.ZERO, get_viewport_rect().size), BACKGROUND_COLOR, true)
	_draw_label("MOLTKE ENGINE", Vector2(70, 54), 30, Color(0.93, 0.95, 0.98))
	_draw_label(
		"World Map prototype  |  Shared province data  |  Click a province",
		Vector2(72, 88),
		16,
		Color(0.66, 0.72, 0.79)
	)

	if not _error_text.is_empty():
		_draw_label(_error_text, Vector2(72, 150), 18, Color(1.0, 0.55, 0.45))
		return
	if _world_map == null:
		_draw_label("World data is not available.", Vector2(72, 150), 18, Color.WHITE)
		return

	for tile_value in _tiles:
		if not (tile_value is Dictionary):
			continue
		var tile: Dictionary = tile_value
		var province_id := str(tile.get("province_id", ""))
		var province := _world_map.get_province(province_id)
		if province == null:
			continue
		_draw_province_tile(province, _tile_rect(tile))

	var selected := _world_map.get_province(_selected_province_id)
	if selected != null:
		var region := _world_map.get_region(selected.region_id)
		var region_name := region.name if region != null else selected.region_id
		var adjacent_count := _world_map.get_adjacent_provinces(selected.id).size()
		_draw_label(
			"Selected: %s    Region: %s    Adjacent provinces: %d"
			% [selected.name, region_name, adjacent_count],
			Vector2(70, 590),
			18,
			Color(0.94, 0.96, 0.99)
		)
	else:
		_draw_label("Select a province to inspect its region and neighbours.", Vector2(70, 590), 18, Color(0.78, 0.82, 0.88))

	_draw_label(
		"Demo layout is separate from simulation data. 2D/3D renderers will consume the same WorldMapData.",
		Vector2(70, 645),
		14,
		Color(0.58, 0.65, 0.73)
	)


func _draw_province_tile(province: ProvinceData, rect: Rect2) -> void:
	var fill_color: Color = OWNER_COLORS.get(province.owner_id, Color(0.32, 0.36, 0.42))
	draw_rect(rect, PANEL_COLOR, true)
	draw_rect(rect, fill_color, true)
	draw_rect(rect, Color(0.035, 0.045, 0.06, 0.30), false, 2.0)

	if province.id == _selected_province_id:
		draw_rect(rect.grow(2.0), Color(0.96, 0.92, 0.70), false, 4.0)
	else:
		draw_rect(rect, BORDER_COLOR, false, 1.5)

	var text_color := Color(0.97, 0.98, 1.0)
	var detail_color := Color(0.90, 0.93, 0.96)
	_draw_label(province.name, rect.position + Vector2(18, 38), 22, text_color)
	_draw_label("Owner: %s" % province.owner_id.replace("_", " ").capitalize(), rect.position + Vector2(18, 78), 15, detail_color)
	_draw_label("Terrain: %s" % province.terrain_type, rect.position + Vector2(18, 105), 15, detail_color)
	_draw_label("Population: %s" % _format_population(province.population), rect.position + Vector2(18, 132), 15, detail_color)


func _format_population(value: int) -> String:
	var digits := str(value)
	var formatted := ""
	for index in range(digits.length() - 1, -1, -1):
		if not formatted.is_empty() and (digits.length() - 1 - index + 1) % 3 == 0:
			formatted = "," + formatted
		formatted = digits.substr(index, 1) + formatted
	return formatted


func _draw_label(text: String, position: Vector2, font_size: int, color: Color) -> void:
	draw_string(
		ThemeDB.fallback_font,
		position,
		text,
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		font_size,
		color
	)
