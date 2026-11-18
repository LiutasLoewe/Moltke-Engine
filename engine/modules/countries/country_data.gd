extends RefCounted
class_name CountryData

## Stable country identity and game-facing political data.
var tag: String = ""
var display_name: String = ""
var government_type_id: String = ""
var stability: int = 50
var core_province_ids: Array[String] = []


static func from_dictionary(data: Dictionary) -> CountryData:
	var country := CountryData.new()
	country.tag = str(data.get("tag", "")).strip_edges().to_upper()
	country.display_name = str(data.get("display_name", country.tag)).strip_edges()
	country.government_type_id = str(data.get("government_type_id", "")).strip_edges()
	country.stability = int(data.get("stability", 50))

	var raw_cores: Variant = data.get("core_province_ids", [])
	if raw_cores is Array:
		for province_id in raw_cores:
			country.core_province_ids.append(str(province_id).strip_edges())
	return country
