extends TextEdit
class_name TraceTextEdit

@export_category("Node References")
@export var _can_bridge: GodotCanBridge

var _v_scroll: VScrollBar
var _auto_scroll := true
var _ignore_scroll_changes := true

const TIMESTAMP_IDX = 0
const CAN_ID_IDX = 1
const LENGTH_IDX = 2
const DATA_START_IDX = 4
const MAX_NUMBER_OF_LINES = 100

func _ready() -> void:
	_v_scroll = get_v_scroll_bar()
	_v_scroll.value_changed.connect(_on_scroll_changed)


func _on_scroll_changed(value: float) -> void:
	if _ignore_scroll_changes:
		return
	
	# If the scrollbar is at the bottom (or very close), we auto-scroll to remain at the bottom
	_auto_scroll = value >= _v_scroll.max_value - get_visible_line_count() - 1


func _process(_delta: float) -> void:
	if _can_bridge.is_alive():
		for frame in _can_bridge.get_recent_can_msgs():
			_append_text(_format_raw_frame(frame))


func _append_text(line: String) -> void:
	# Ignore the 'value_changed' scroll calls that are automatically called when updating text, we only want to redetermine _auto_scroll from the user's scrolls
	_ignore_scroll_changes = true

	var prev_scroll_value = _v_scroll.value
	self.text += line + "\n"

	# If we have too many lines, delete the oldest lines
	while get_line_count() > MAX_NUMBER_OF_LINES:
		remove_line_at(0)
		prev_scroll_value -= 1

	if _auto_scroll:
		# Scroll to bottom
		_v_scroll.value = self.get_line_count()
	else:
		# Hold scroll position
		_v_scroll.value = prev_scroll_value
	
	_ignore_scroll_changes = false


func _format_raw_frame(frame: Array) -> String:
	var frame_text: String = ""
	frame_text += ("%08d " % int(frame[TIMESTAMP_IDX]))
	frame_text += ("%03x " % int(frame[CAN_ID_IDX])).to_upper()
	frame_text += ("[%01d] " % int(frame[LENGTH_IDX]))
	for data_idx in range(DATA_START_IDX, frame.size()):
		frame_text += ("%02x " % int(frame[data_idx]))

	return frame_text
