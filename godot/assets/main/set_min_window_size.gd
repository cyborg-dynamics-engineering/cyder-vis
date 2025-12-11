## Sets the minimum window size of the DisplayServer running the project
extends Node

const MIN_X: int = 700
const MIN_Y: int = 400


func _ready() -> void:
	DisplayServer.window_set_min_size(Vector2i(MIN_X, MIN_Y))
