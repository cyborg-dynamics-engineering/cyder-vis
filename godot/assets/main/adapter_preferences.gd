class_name AdapterPreferences
extends RefCounted

var path := "user://adapter_preferences.cfg"
var _config := ConfigFile.new()


func load_preferences() -> Error:
	var error := _config.load(path)
	return OK if error == ERR_FILE_NOT_FOUND else error


func nickname(identity: String) -> String:
	return str(_config.get_value("nicknames", identity, "")) if not identity.is_empty() else ""


func selected(driver: String) -> String:
	return str(_config.get_value("selected", driver, ""))


func save_nickname(identity: String, value: String) -> Error:
	if identity.is_empty():
		return ERR_INVALID_PARAMETER
	var previous := nickname(identity)
	_config.set_value("nicknames", identity, value.strip_edges())
	var error := _config.save(path)
	if error != OK:
		_config.set_value("nicknames", identity, previous)
	return error


func save_selection(driver: String, identity: String) -> Error:
	var previous := selected(driver)
	_config.set_value("selected", driver, identity)
	var error := _config.save(path)
	if error != OK:
		_config.set_value("selected", driver, previous)
	return error


func record_interface(name: String, identity: String, device_name: String, driver: String, device: String, label := "") -> Error:
	_config.set_value("interfaces", name, {"identity": identity, "device_name": device_name, "driver": driver, "device": device, "label": label})
	return _config.save(path)


func interface_record(name: String) -> Dictionary:
	return _config.get_value("interfaces", name, {})


func display_name(interface: Dictionary) -> String:
	var record := interface_record(interface.name)
	if record.get("driver", "") == interface.driver and record.get("device", "") == interface.device:
		var name := nickname(record.get("identity", ""))
		var label: String = record.get("label", "")
		if label.is_empty():
			label = record.get("device_name", interface.name)
		return label if name.is_empty() else name + " — " + label
	if interface.driver == "slcan":
		return interface.device
	return str(interface.name)
