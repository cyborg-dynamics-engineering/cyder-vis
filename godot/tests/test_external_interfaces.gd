extends SceneTree


class TestControls extends InterfaceControls:
	var refreshes := 0

	func _refresh_adapters() -> void:
		refreshes += 1


func _initialize() -> void:
	_run.call_deferred()


func _adapter(selector: String, aliases: Array = [], available := true) -> Dictionary:
	return {"identity": "gsusb:" + selector, "selector": selector, "aliases": aliases,
		"label": selector, "available": available, "reason": "In use by another server" if not available else ""}


func _service(device: String, state := "up", driver := "gsusb") -> Dictionary:
	return {"name": "manual_service", "driver": driver, "device": device, "state": state, "bitrate": 500000, "bus_load": 0.0}


func _run() -> void:
	var controls := TestControls.new()
	controls._windows = true
	controls.discovery_enabled = false
	controls._preferences.path = "user://unused_external_interfaces_%s.cfg" % Time.get_ticks_usec()
	root.add_child(controls)
	var adapters := [_adapter("SERIAL/0", ["SERIAL/0", "SERIAL", "001:002/0", "001:002"]), _adapter("OTHER/0", ["OTHER/0", "OTHER", "001:003/0", "001:003"])]
	controls._accept_scan({"driver": "gsusb", "adapters": adapters})
	# No saved interface names or local ownership: every selector identifies the
	# externally started service by its physical device, including CLI aliases.
	for device: String in ["SERIAL/0", "serial", "001:002/0", "001:002"]:
		controls.set_running_interfaces([_service(device)])
		assert(controls._adapter.is_item_disabled(1), device)
		assert(controls._adapter.get_item_tooltip(1) == "Already running")
		assert(not controls._adapter.is_item_disabled(2))
		controls._adapter.select(1)
		controls._process(0.0)
		assert(controls._actions[0].disabled)
		assert(controls._actions[0].tooltip_text == "The interface is already up.")
	# Matching must prefer an up service if an old service reports down first.
	var down := _service("SERIAL/0", "down")
	down.name = "old_service"
	controls.set_running_interfaces([down, _service("SERIAL/0")])
	assert(controls._adapter.is_item_disabled(1))
	# Service state changes request a fresh hardware scan, not bus-load updates.
	controls.set_running_interfaces([_service("SERIAL/0")])
	var refreshes := controls.refreshes
	var service := _service("SERIAL/0")
	service.bus_load = 35.0
	controls.set_running_interfaces([service])
	assert(controls.refreshes == refreshes)
	service.state = "down"
	controls.set_running_interfaces([service])
	assert(controls.refreshes == refreshes + 1)
	assert(not controls._adapter.is_item_disabled(1))
	# An auto-selected/busy adapter may not expose its serial. Its discovery
	# result still greys that physical entry without disabling the free adapter.
	controls.set_running_interfaces([_service("auto")])
	controls._accept_scan({"driver": "gsusb", "adapters": [_adapter("001:002/0", ["001:002"], false), adapters[1]]})
	assert(controls._adapter.is_item_disabled(1))
	assert(controls._adapter.get_item_tooltip(1).contains("In use by another server"))
	assert(not controls._adapter.is_item_disabled(2))
	controls.set_running_interfaces([])
	controls._accept_scan({"driver": "gsusb", "adapters": adapters})
	assert(not controls._adapter.is_item_disabled(1))
	# A serial can address multiple USB channels. Do not guess which channel a
	# bare-serial service selected; the actual busy result identifies it.
	controls._accept_scan({"driver": "gsusb", "adapters": [adapters[0], _adapter("SERIAL/1", ["SERIAL/1", "SERIAL", "001:002/1", "001:002"])]})
	controls.set_running_interfaces([_service("SERIAL")])
	assert(not controls._adapter.is_item_disabled(1) and not controls._adapter.is_item_disabled(2))
	controls.set_running_interfaces([_service("SERIAL/1")])
	assert(not controls._adapter.is_item_disabled(1) and controls._adapter.is_item_disabled(2))
	for pair in [[1, "slcan", "COM7", "\\\\.\\com7"], [2, "pcan", "USBBUS1", "usbbus1"]]:
		controls._driver.select(pair[0])
		controls._driver_changed()
		controls._accept_scan({"driver": pair[1], "adapters": [_adapter(pair[2])]})
		controls.set_running_interfaces([_service(pair[3], "up", pair[1])])
		assert(controls._adapter.is_item_disabled(1))
		controls.set_running_interfaces([])
		assert(not controls._adapter.is_item_disabled(1))
	controls.free()
	await process_frame
	print("External interface dropdown and device alias tests passed")
	quit()
