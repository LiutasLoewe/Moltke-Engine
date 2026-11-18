extends RefCounted
class_name CountryRegistry

## Registry for country tags. Province ownership continues to use these stable tags.
var countries: Dictionary = {}
var validation_errors: Array[String] = []


func load_from_dictionary(payload: Dictionary, world_map: WorldMapData = null) -> bool:
	countries.clear()
	validation_errors.clear()

	var raw_countries: Variant = payload.get("countries", null)
	if not (raw_countries is Array):
		validation_errors.append("The 'countries' field must be an array.")
		return false

	var candidate_countries: Dictionary = {}
	for raw_country in raw_countries:
		if not (raw_country is Dictionary):
			validation_errors.append("Every country entry must be an object.")
			continue

		var country: CountryData = CountryData.from_dictionary(raw_country)
		if not _is_valid_tag(country.tag):
			validation_errors.append("Country tag '%s' must be 2-8 uppercase letters or digits." % country.tag)
			continue
		if candidate_countries.has(country.tag):
			validation_errors.append("Duplicate country tag: %s" % country.tag)
			continue
		if country.display_name.is_empty():
			validation_errors.append("Country %s is missing its display name." % country.tag)
		if country.government_type_id.is_empty():
			validation_errors.append("Country %s is missing its government_type_id." % country.tag)
		if country.stability < 0 or country.stability > 100:
			validation_errors.append("Country %s stability must be between 0 and 100." % country.tag)

		var seen_cores: Dictionary = {}
		for province_id in country.core_province_ids:
			if province_id.is_empty():
				validation_errors.append("Country %s has an empty core province ID." % country.tag)
			elif seen_cores.has(province_id):
				validation_errors.append(
					"Country %s lists core province %s more than once." % [country.tag, province_id]
				)
			elif world_map != null and world_map.get_province(province_id) == null:
				validation_errors.append(
					"Country %s references unknown core province %s." % [country.tag, province_id]
				)
			seen_cores[province_id] = true

		candidate_countries[country.tag] = country

	if world_map != null:
		for province_id in world_map.provinces.keys():
			var province: ProvinceData = world_map.get_province(str(province_id))
			if not province.owner_id.is_empty() and not candidate_countries.has(province.owner_id):
				validation_errors.append(
					"Province %s references unknown country tag %s." % [province.id, province.owner_id]
				)

	if validation_errors.is_empty():
		countries = candidate_countries
	return validation_errors.is_empty()


func get_country(tag: String) -> CountryData:
	return countries.get(tag.to_upper()) as CountryData


func get_country_tags() -> Array[String]:
	var tags: Array[String] = []
	for tag in countries.keys():
		tags.append(str(tag))
	tags.sort()
	return tags


func get_owned_provinces(tag: String, world_map: WorldMapData) -> Array[ProvinceData]:
	var result: Array[ProvinceData] = []
	var owner_tag := tag.to_upper()
	for province_id in world_map.provinces.keys():
		var province: ProvinceData = world_map.get_province(str(province_id))
		if province.owner_id == owner_tag:
			result.append(province)
	result.sort_custom(func(left: ProvinceData, right: ProvinceData) -> bool:
		return left.id < right.id
	)
	return result


func _is_valid_tag(tag: String) -> bool:
	if tag.length() < 2 or tag.length() > 8:
		return false
	for character in tag:
		var codepoint := character.unicode_at(0)
		var is_uppercase_letter := codepoint >= 65 and codepoint <= 90
		var is_digit := codepoint >= 48 and codepoint <= 57
		if not is_uppercase_letter and not is_digit:
			return false
	return true
