extends TextEdit
class_name TraceTextEdit

@export_category("Node References")
@export var _can_bridge: GodotCanBridge

var _v_scroll: VScrollBar


func _ready() -> void:
	_v_scroll = get_v_scroll_bar()


func _process(_delta: float) -> void:
	if _can_bridge.is_alive():
		_set_text(_can_bridge.get_recent_can_msgs())


func _set_text(new_text: String) -> void:
	var prev_scroll_value = _v_scroll.value

	# Add the text
	self.text = new_text

	# Hold scroll position
	_v_scroll.value = prev_scroll_value
