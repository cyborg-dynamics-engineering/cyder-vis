extends VBoxContainer
class_name RecordingControls

@export var can_bridge: GodotCanBridge

var _directory: LineEdit
var _mode: OptionButton
var _csv: CheckBox
var _mf4: CheckBox
var _start: Button
var _stop: Button
var _browse: Button
var _status: Label
var _was_recording: bool = false


func _ready() -> void:
	var path_row := HBoxContainer.new()
	add_child(path_row)
	_directory = LineEdit.new()
	_directory.text = DbcFileButton.home_directory()
	_directory.placeholder_text = "Recording directory"
	_directory.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	path_row.add_child(_directory)
	_browse = Button.new()
	_browse.text = "Recording folder…"
	_browse.pressed.connect(_choose_directory)
	path_row.add_child(_browse)
	var options := HFlowContainer.new()
	add_child(options)
	_mode = OptionButton.new()
	for mode in ["Raw frames", "Decoded signals", "Raw + decoded (separate files)"]:
		_mode.add_item(mode)
	options.add_child(_mode)
	var format_group := ButtonGroup.new()
	_csv = CheckBox.new()
	_csv.text = "CSV"
	_csv.button_group = format_group
	_csv.button_pressed = true
	options.add_child(_csv)
	_mf4 = CheckBox.new()
	_mf4.text = "MF4"
	_mf4.tooltip_text = "Requires Python, python-can[mf4] and asammdf."
	_mf4.button_group = format_group
	options.add_child(_mf4)
	_start = Button.new()
	_start.text = "Start recording"
	_start.pressed.connect(_start_recording)
	options.add_child(_start)
	_stop = Button.new()
	_stop.text = "Stop recording"
	_stop.pressed.connect(_stop_recording)
	options.add_child(_stop)
	_status = Label.new()
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	add_child(_status)


func _process(_delta: float) -> void:
	var active: bool = can_bridge.is_recording()
	if active and not can_bridge.is_alive():
		_stop_recording()
		active = false
	_start.disabled = active or not can_bridge.is_alive()
	_stop.disabled = not active
	_mode.disabled = active
	_csv.disabled = active
	_mf4.disabled = active
	_browse.disabled = active
	_directory.editable = not active
	if _was_recording and not active and _status.text.begins_with("Recording: "):
		_status.text = "Recording stopped. " + _status.text.trim_prefix("Recording: ")
	_was_recording = active


func _choose_directory() -> void:
	var dialog := FileDialog.new()
	dialog.access = FileDialog.ACCESS_FILESYSTEM
	dialog.file_mode = FileDialog.FILE_MODE_OPEN_DIR
	dialog.current_dir = _directory.text
	add_child(dialog)
	dialog.dir_selected.connect(func(path: String):
		_directory.text = path
		dialog.queue_free())
	dialog.canceled.connect(dialog.queue_free)
	dialog.popup_centered_ratio(0.7)


func _start_recording() -> void:
	var error: String = can_bridge.start_recording(_directory.text, _mode.selected, "mf4" if _mf4.button_pressed else "csv")
	if not error.is_empty():
		AlertHandler.display_error(error)
		return
	_status.text = "Recording: " + can_bridge.recording_files().replace("\n", " | ")
	_status.tooltip_text = can_bridge.recording_files()


func _stop_recording() -> void:
	_status.text = "Finalizing recording…"
	var error: String = can_bridge.stop_recording()
	_status.text = "Recording saved. " + _status.tooltip_text.replace("\n", " | ")
	if not error.is_empty():
		_status.text = "Recording failed: " + error
		AlertHandler.display_error(error)
