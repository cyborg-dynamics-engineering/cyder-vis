extends SceneTree


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var scene: Node = load("res://assets/main/main.tscn").instantiate()
	root.add_child(scene)
	current_scene = scene
	var controls: RecordingControls = scene.get_node("Background/VSplitContainer/TabContainer/Trace/MarginContainer/VBoxContainer/RecordingControls")
	assert(controls._csv.button_pressed and not controls._mf4.button_pressed)
	assert(controls._csv.button_group == controls._mf4.button_group)
	controls._mf4.button_pressed = true
	assert(controls._mf4.button_pressed and not controls._csv.button_pressed)
	controls._csv.button_pressed = true
	assert(controls._csv.button_pressed and not controls._mf4.button_pressed)
	assert(controls._mode.item_count == 3)
	await process_frame
	await process_frame
	assert(controls._start.disabled and controls._stop.disabled)
	assert(not controls.can_bridge.start_recording(controls._directory.text, 0, "csv").is_empty())
	scene.queue_free()
	await process_frame
	print("Recording control radio and state tests passed")
	quit()
