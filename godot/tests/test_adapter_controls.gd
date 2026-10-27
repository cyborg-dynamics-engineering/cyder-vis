extends SceneTree

var _path := "user://test_adapter_preferences_%s.cfg" % Time.get_ticks_usec()


func _initialize() -> void:
	_run.call_deferred()


func _controls() -> InterfaceControls:
	var controls := InterfaceControls.new()
	controls.discovery_enabled = false
	controls._windows = true
	controls._preferences.path = _path
	root.add_child(controls)
	return controls


func _adapter(identity: String, selector: String, available := true) -> Dictionary:
	return {"identity": identity, "selector": selector, "label": "Adapter at " + selector,
		"identity_note": "Persistent hardware identity", "available": available, "reason": "Busy" if not available else ""}


func _run() -> void:
	var first := _controls()
	first._accept_scan({"driver": "gsusb", "adapters": [_adapter("gsusb:SERIAL/0", "SERIAL/0"), _adapter("gsusb:OTHER/0", "OTHER/0")]})
	assert(first._adapter.item_count == 3)
	assert(first._selection().is_empty()) # Multiple devices require a choice.
	first._adapter.select(1)
	first._adapter_selected(1)
	first._nickname.text = "Powertrain bench"
	first._save_nickname()
	assert(first._adapter.get_item_text(1).begins_with("Powertrain bench"))
	first.free()

	# New controls reload preferences from disk, as after restarting the application.
	var second := _controls()
	var moved := _adapter("gsusb:SERIAL/0", "SERIAL/0")
	moved.label = "Adapter at a new USB address"
	second._accept_scan({"driver": "gsusb", "adapters": [_adapter("gsusb:OTHER/0", "OTHER/0"), moved]})
	assert(second._adapter.selected == 2)
	assert(second._nickname.text == "Powertrain bench")
	assert(not second._adapter.get_item_text(1).contains("Powertrain bench"))
	# Never silently substitute another device if the saved serial has disappeared.
	second._accept_scan({"driver": "gsusb", "adapters": [_adapter("gsusb:OTHER/0", "OTHER/0")]})
	assert(second._selection().is_empty())
	await process_frame
	await process_frame
	assert(second._actions[0].disabled)

	second._driver.select(2)
	second._driver_changed()
	second._accept_scan({"driver": "gsusb", "adapters": [moved]})
	assert(second._adapters.is_empty()) # Ignore an old driver's asynchronous result.
	second._accept_scan({"driver": "pcan", "adapters": [_adapter("pcan:guid:abc/0", "USBBUS1")]})
	second._nickname.text = "PEAK bench"
	second._save_nickname()
	second._adapter_selected(1)
	second._accept_scan({"driver": "pcan", "adapters": [_adapter("pcan:guid:abc/0", "USBBUS3", false)]})
	assert(second._nickname.text == "PEAK bench")
	await process_frame
	await process_frame
	assert(second._actions[0].disabled)
	second._nickname.text = ""
	second._save_nickname()
	assert(second._preferences.nickname("pcan:guid:abc/0").is_empty())

	second._driver.select(1)
	second._driver_changed()
	second._accept_scan({"driver": "slcan", "adapters": [_adapter("slcan:COM7", "COM7")]})
	assert(not second._nickname_row.visible)
	assert(second._selection().selector == "COM7")
	second._accept_scan({"driver": "slcan", "adapters": []})
	assert(second._selection().is_empty())
	second.free()
	var preferences := AdapterPreferences.new()
	preferences.path = _path
	assert(preferences.load_preferences() == OK)
	assert(preferences.nickname("gsusb:SERIAL/0") == "Powertrain bench")
	assert(preferences.save_nickname("", "Invalid") == ERR_INVALID_PARAMETER)
	preferences.path = "user://missing_parent/preferences.cfg"
	assert(preferences.save_nickname("gsusb:SERIAL/0", "Unsaved") != OK)
	assert(preferences.nickname("gsusb:SERIAL/0") == "Powertrain bench")
	DirAccess.remove_absolute(_path)
	print("Adapter dropdown, persistence and state tests passed")
	quit()
