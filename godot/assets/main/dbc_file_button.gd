extends Button
class_name DbcFileButton

@export_category("Node References")
@export var _can_bridge: GodotCanBridge
@export var _dbc_file_box: LineEdit
@export var _receive_table: ReceiveTable


func _ready() -> void:
	self.pressed.connect(_button_pressed)


func _button_pressed() -> void:
	var file_dialog = FileDialog.new()
	add_child(file_dialog)
	file_dialog.set_file_mode(file_dialog.FILE_MODE_OPEN_FILE)
	file_dialog.access = FileDialog.ACCESS_FILESYSTEM
	file_dialog.filters = ["*.dbc; CAN DBC Files"]
	file_dialog.popup()
	file_dialog.file_selected.connect(_process_file)


func _process_file(x: String) -> void:
	var dbc_success = _can_bridge.load_dbc_file(x) # This emits an alert if bad file
	if dbc_success:
		_dbc_file_box.text = x
		_receive_table.clear_all()
