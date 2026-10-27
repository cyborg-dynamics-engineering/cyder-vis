extends SceneTree


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var scene: Node = load("res://assets/main/main.tscn").instantiate()
	var panel: RunningInterfaces = scene.get_node("Background/VSplitContainer/TabContainer/Interface/RunningInterfaces")
	panel.discovery_enabled = false
	panel.setup.discovery_enabled = false
	panel.setup._windows = true
	panel.setup._preferences.path = "user://test_running_interfaces_%s.cfg" % Time.get_ticks_usec()
	root.add_child(scene)
	current_scene = scene
	assert(scene.get_node_or_null("Background/VSplitContainer/TabContainer/Interface/CanInterfaceBox") == null)
	assert(not panel.connection.visible)
	assert(not panel.setup_dialog.visible)
	assert(panel.setup._actions.size() == 1 and panel.setup._actions[0].text == "Start")
	panel.setup_dialog.popup_centered(Vector2i(680, 330))
	await process_frame
	await process_frame
	assert(panel.setup_dialog.size.y <= 400)
	panel.setup_dialog.hide()
	panel.setup._preferences.record_interface("Powertrain", "gsusb:SERIAL/0", "Cyder SH-C31A gs_usb", "gsusb", "SERIAL/0")
	panel.setup._preferences.save_nickname("gsusb:SERIAL/0", "Powertrain")
	var interfaces := [
		{"name": "Powertrain", "driver": "gsusb", "device": "SERIAL/0", "bitrate": 500000, "bus_load": 12.5, "state": "up"},
		{"name": "COM7", "driver": "slcan", "device": "COM7", "bitrate": 250000, "bus_load": 0.0, "state": "up"}]
	panel._update_interfaces(interfaces)
	assert(panel._rows.Powertrain.name_label.text == "Powertrain — Cyder SH-C31A gs_usb")
	assert(panel._rows.COM7.name_label.text == "COM7")
	assert(panel._rows.Powertrain.bitrate.text == "500000 bit/s")
	assert(panel._rows.Powertrain.bus_load.text == "12.50%")
	assert(panel._rows.COM7.bus_load.text == "0.00%")
	panel._update_connection_buttons("Powertrain")
	assert(panel._rows.Powertrain.connect.text == "Disconnect")
	assert(not panel._rows.Powertrain.connect.disabled)
	assert(panel._rows.COM7.connect.disabled)
	assert(not panel._rows.COM7.stop.disabled)
	panel._update_connection_buttons("")
	assert(not panel._rows.Powertrain.connect.disabled and not panel._rows.COM7.connect.disabled)
	panel.setup._accept_scan({"driver": "gsusb", "adapters": [{"identity": "gsusb:SERIAL/0", "selector": "SERIAL/0", "label": "Cyder", "device_name": "Cyder", "available": true}]})
	await process_frame
	await process_frame
	assert(panel.setup._actions[0].disabled)
	assert(panel.setup._adapter.is_item_disabled(1))
	assert(panel.setup._adapter.get_item_tooltip(1) == "Already running")
	assert(panel.setup._selection().is_empty()) # Do not restore a saved running adapter.
	interfaces[0].state = "down"
	panel._update_interfaces(interfaces)
	assert(not panel.setup._adapter.is_item_disabled(1))
	assert(not panel.setup._adapter.get_item_tooltip(1).contains("Already running"))
	panel.setup._adapter.select(1)
	assert(panel.setup._selection().selector == "SERIAL/0")
	interfaces[0].state = "up"
	panel._update_interfaces(interfaces)
	assert(panel.setup._adapter.is_item_disabled(1))
	assert(panel.setup._selection().is_empty()) # Clear a selection that starts running.
	panel.setup._driver.select(1)
	panel.setup._driver_changed()
	panel.setup._accept_scan({"driver": "slcan", "adapters": [{"identity": "slcan:COM7", "selector": "COM7", "label": "COM7", "available": true}, {"identity": "slcan:COM8", "selector": "COM8", "label": "COM8", "available": true}]})
	assert(panel.setup._adapter.is_item_disabled(1))
	assert(panel.setup._adapter.get_item_tooltip(1) == "Already running")
	assert(not panel.setup._adapter.is_item_disabled(2))
	assert(panel.setup._adapter.selected == 2) # Only the unused adapter can be auto-selected.
	panel._update_interfaces([])
	assert(not panel.setup._adapter.is_item_disabled(1))
	assert(panel._rows.is_empty() and panel._empty.visible)
	DirAccess.remove_absolute(panel.setup._preferences.path)
	scene.queue_free()
	await process_frame
	print("Running interface list, popup and exclusive connection tests passed")
	quit()
