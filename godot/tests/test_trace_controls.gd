extends SceneTree


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var bridge := GodotCanBridge.new()
	root.add_child(bridge)
	var controls := TraceControls.new()
	controls.can_bridge = bridge
	root.add_child(controls)
	assert(bridge.get_timestamp_epoch() == 1)
	assert(controls._epochs.size() == 3)
	assert(controls._epochs[1].button_pressed)
	assert(not controls._hardware.visible)
	for mode in 3:
		controls._epochs[mode].pressed.emit()
		assert(bridge.get_timestamp_epoch() == mode)
		for index in 3:
			assert(controls._epochs[index].button_pressed == (index == mode))
	assert(not bridge.set_timestamp_epoch(-1).is_empty())
	assert(bridge.get_timestamp_epoch() == 2)
	for status in range(1, 5):
		controls._update_hardware_indicator(status)
		assert(controls._hardware.visible)
		assert(controls._hardware.text.contains("enabled") == (status == 3))
	controls._update_hardware_indicator(0)
	assert(not controls._hardware.visible)
	controls.queue_free()
	bridge.queue_free()
	await process_frame
	print("Trace epoch radio and hardware indicator tests passed")
	quit()
