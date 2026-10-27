extends Button
class_name DbcFileButton

@export_category("Node References")
@export var _can_bridge: GodotCanBridge
@export var _dbc_file_box: LineEdit
@export var _receive_table: ReceiveTable
@export var _transmit_table: TransmitTable


func _ready() -> void:
	self.pressed.connect(_button_pressed)
	_dbc_file_box.text_changed.connect(_dbc_text_changed)
	_dbc_file_box.text_submitted.connect(_process_file)


func _button_pressed() -> void:
	var file_dialog := create_file_dialog()
	add_child(file_dialog)
	file_dialog.popup_centered_ratio(0.7)


static func home_directory() -> String:
	var home := OS.get_environment("USERPROFILE" if OS.get_name() == "Windows" else "HOME")
	if home.is_empty() or not DirAccess.dir_exists_absolute(home):
		home = OS.get_system_dir(OS.SYSTEM_DIR_DOCUMENTS)
	return home.replace("\\", "/")


func create_file_dialog() -> FileDialog:
	var file_dialog := FileDialog.new()
	file_dialog.set_file_mode(file_dialog.FILE_MODE_OPEN_FILE)
	file_dialog.access = FileDialog.ACCESS_FILESYSTEM
	file_dialog.current_dir = home_directory()
	file_dialog.filters = ["*.dbc; CAN DBC Files"]
	file_dialog.file_selected.connect(_process_file)
	file_dialog.file_selected.connect(func(_path: String): file_dialog.queue_free())
	file_dialog.canceled.connect(file_dialog.queue_free)
	return file_dialog


func _process_file(x: String) -> void:
	var dbc_success = _can_bridge.load_dbc_file(x) # This emits an alert if bad file
	if dbc_success:
		_dbc_file_box.text = x
		_dbc_text_changed(x) # Setting the text variable doesn't trigger this signal, so trigger manually
		_transmit_table.refresh_dbc()


func _dbc_text_changed(new_text: String):
	if new_text.strip_edges().is_empty():
		_can_bridge.load_dbc_file("")
		_transmit_table.refresh_dbc()
	# Clear the receive table as a different DBC file may require certain messages to be redrawn as deserialised/serialised versions
	_receive_table.clear_all()
