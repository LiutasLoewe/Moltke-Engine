extends RefCounted
class_name ProvinceData

## Simulation data for one province. Renderer-specific geometry is stored separately.

var id: String = ""
var name: String = ""
var owner_id: String = ""
var region_id: String = ""
var terrain_type: String = "unknown"
var population: int = 0
var adjacent_province_ids: Array[String] = []


static func from_dictionary(data: Dictionary) -> ProvinceData:
	var province := ProvinceData.new()
	province.id = str(data.get("id", "")).strip_edges()
	province.name = str(data.get("name", province.id)).strip_edges()
	province.owner_id = str(data.get("owner_id", "")).strip_edges()
	province.region_id = str(data.get("region_id", "")).strip_edges()
	province.terrain_type = str(data.get("terrain_type", "unknown")).strip_edges()
	province.population = int(data.get("population", 0))

	var raw_adjacencies: Variant = data.get("adjacent_province_ids", [])
	if raw_adjacencies is Array:
		for adjacent_id in raw_adjacencies:
			province.adjacent_province_ids.append(str(adjacent_id).strip_edges())

	return province
