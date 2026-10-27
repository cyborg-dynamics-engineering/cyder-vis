extends SceneTree


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var handler := AlertHandler.new()
	root.add_child(handler)
	assert(current_scene == null)
	AlertHandler.display_error("Missing Linux can-utils tools: candump, cansend. Install can-utils.")
	assert(is_instance_valid(handler.prev_alert_box))
	assert(handler.prev_alert_box.dialog_text.contains("Install can-utils"))
	handler.queue_free()
	await process_frame
	print("Startup warning tests passed")
	quit()
