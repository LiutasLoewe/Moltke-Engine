extends RefCounted
class_name SaveGameSerializer

## Versioned JSON envelope for JSON-compatible simulation and module state.
const FORMAT_ID := "moltke_save"
const FORMAT_VERSION := 1


static func save_file(path: String, snapshot: Dictionary) -> Dictionary:
	if path.strip_edges().is_empty():
		return {"ok": false, "error": "A save path is required."}

	var envelope := {
		"format": FORMAT_ID,
		"format_version": FORMAT_VERSION,
		"snapshot": snapshot.duplicate(true)
	}
	var file := FileAccess.open(path, FileAccess.WRITE)
	if file == null:
		return {
			"ok": false,
			"error": "Could not open save file (error %d)." % FileAccess.get_open_error()
		}

	file.store_string(JSON.stringify(envelope, "\t"))
	file.flush()
	var write_error := file.get_error()
	file.close()
	if write_error != OK:
		return {"ok": false, "error": "Could not finish writing the save file (error %d)." % write_error}
	return {"ok": true, "error": ""}


static func load_file(path: String) -> Dictionary:
	if not FileAccess.file_exists(path):
		return {"ok": false, "error": "Save file does not exist: %s" % path}

	var file := FileAccess.open(path, FileAccess.READ)
	if file == null:
		return {
			"ok": false,
			"error": "Could not open save file (error %d)." % FileAccess.get_open_error()
		}

	var source := file.get_as_text()
	file.close()
	var parser := JSON.new()
	var parse_error := parser.parse(source)
	if parse_error != OK:
		return {
			"ok": false,
			"error": "Invalid save JSON at line %d: %s"
				% [parser.get_error_line(), parser.get_error_message()]
		}

	var envelope: Variant = parser.data
	if not (envelope is Dictionary):
		return {"ok": false, "error": "The save root must be a JSON object."}
	if str(envelope.get("format", "")) != FORMAT_ID:
		return {"ok": false, "error": "This file is not a Moltke save."}
	if typeof(envelope.get("format_version", null)) != TYPE_INT:
		return {"ok": false, "error": "The save is missing its format version."}
	if int(envelope["format_version"]) != FORMAT_VERSION:
		return {
			"ok": false,
			"error": "Unsupported save format version: %d" % int(envelope["format_version"])
		}

	var snapshot: Variant = envelope.get("snapshot", null)
	if not (snapshot is Dictionary):
		return {"ok": false, "error": "The save snapshot must be a JSON object."}
	return {"ok": true, "error": "", "snapshot": snapshot}
