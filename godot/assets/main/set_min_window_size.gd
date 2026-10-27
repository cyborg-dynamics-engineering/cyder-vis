## Sets the minimum window size of the DisplayServer running the project
extends Node

const MIN_X: int = 700
const MIN_Y: int = 400


func _ready() -> void:
	DisplayServer.window_set_min_size(Vector2i(MIN_X, MIN_Y))
	get_tree().auto_accept_quit = false
	var warning: String = $GodotCanBridge.check_dependencies()
	if not warning.is_empty():
		AlertHandler.display_error(warning)


func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		if $GodotCanBridge.is_alive():
			$GodotCanBridge.close_bus()
		var error: String = $GodotCanBridge.stop_recording()
		if not error.is_empty():
			push_error(error)
		get_tree().quit()
