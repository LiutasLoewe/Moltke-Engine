extends RefCounted
class_name WorldMapData

## Authoritative, renderer-independent data for provinces, regions, and adjacency.

var provinces: Dictionary = {}
var regions: Dictionary = {}
var validation_errors: Array[String] = []


func load_from_dictionary(payload: Dictionary) -> bool:
	provinces.clear()
	regions.clear()
	validation_errors.clear()

	var raw_regions: Variant = payload.get("regions", null)
	if not (raw_regions is Array):
		validation_errors.append("The 'regions' field must be an array.")
	else:
		for region_value in raw_regions:
			if not (region_value is Dictionary):
				validation_errors.append("Every region entry must be an object.")
				continue

			var region: RegionData = RegionData.from_dictionary(region_value)
			if region.id.is_empty():
				validation_errors.append("A region is missing its required 'id'.")
			elif regions.has(region.id):
				validation_errors.append("Duplicate region id: %s" % region.id)
			else:
				regions[region.id] = region

	var raw_provinces: Variant = payload.get("provinces", null)
	if not (raw_provinces is Array):
		validation_errors.append("The 'provinces' field must be an array.")
	else:
		for province_value in raw_provinces:
			if not (province_value is Dictionary):
				validation_errors.append("Every province entry must be an object.")
				continue

			var province: ProvinceData = ProvinceData.from_dictionary(province_value)
			if province.id.is_empty():
				validation_errors.append("A province is missing its required 'id'.")
			elif provinces.has(province.id):
				validation_errors.append("Duplicate province id: %s" % province.id)
			elif province.population < 0:
				validation_errors.append("Province %s has a negative population." % province.id)
			else:
				provinces[province.id] = province

	_validate_references()
	return validation_errors.is_empty()


func get_province(province_id: String) -> ProvinceData:
	return provinces.get(province_id) as ProvinceData


func get_region(region_id: String) -> RegionData:
	return regions.get(region_id) as RegionData


func get_adjacent_provinces(province_id: String) -> Array[ProvinceData]:
	var result: Array[ProvinceData] = []
	var province := get_province(province_id)
	if province == null:
		return result

	for adjacent_id in province.adjacent_province_ids:
		var adjacent_province := get_province(adjacent_id)
		if adjacent_province != null:
			result.append(adjacent_province)

	return result


func get_provinces_in_region(region_id: String) -> Array[ProvinceData]:
	var result: Array[ProvinceData] = []
	if not regions.has(region_id):
		return result

	for province_id in provinces.keys():
		var province: ProvinceData = provinces[province_id] as ProvinceData
		if province.region_id == region_id:
			result.append(province)

	return result


func get_province_count() -> int:
	return provinces.size()


func get_region_count() -> int:
	return regions.size()


func _validate_references() -> void:
	for province_id in provinces.keys():
		var province: ProvinceData = provinces[province_id] as ProvinceData

		if not regions.has(province.region_id):
			validation_errors.append(
				"Province %s references unknown region %s." % [province.id, province.region_id]
			)

		var seen_adjacencies: Dictionary = {}
		for adjacent_id in province.adjacent_province_ids:
			var already_seen: bool = seen_adjacencies.has(adjacent_id)
			if adjacent_id == province.id:
				validation_errors.append("Province %s cannot be adjacent to itself." % province.id)
			elif already_seen:
				validation_errors.append(
					"Province %s lists adjacent province %s more than once." % [province.id, adjacent_id]
				)
			elif not provinces.has(adjacent_id):
				validation_errors.append(
					"Province %s references unknown adjacent province %s." % [province.id, adjacent_id]
				)
			else:
				var adjacent_province: ProvinceData = provinces[adjacent_id] as ProvinceData
				if not adjacent_province.adjacent_province_ids.has(province.id):
					validation_errors.append(
						"Adjacency between %s and %s must be declared in both directions."
						% [province.id, adjacent_id]
					)
			seen_adjacencies[adjacent_id] = true
