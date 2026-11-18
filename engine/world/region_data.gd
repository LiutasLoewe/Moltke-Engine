extends RefCounted
class_name RegionData

## Geographic grouping for provinces. Membership is derived from ProvinceData.region_id.

var id: String = ""
var name: String = ""


static func from_dictionary(data: Dictionary) -> RegionData:
	var region := RegionData.new()
	region.id = str(data.get("id", "")).strip_edges()
	region.name = str(data.get("name", region.id)).strip_edges()
	return region
