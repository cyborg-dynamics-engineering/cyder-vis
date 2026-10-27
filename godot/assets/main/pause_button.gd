extends Button
class_name PauseButton

@export_category("Node References")
@export var _can_bridge: GodotCanBridge
@export var _tab_container: TabContainer
@export var _receive_table: ReceiveTable
@export var _dbc_line_edit: LineEdit
@export var _dbc_open_button: Button
@export var _status_bar: StatusBar
@export var _transmit_table: TransmitTable

var _is_paused: bool


func _ready() -> void:
	_is_paused = true
	_update_text()
	_update_tab_selectability()
	_update_dbc_editability()


func _process(_delta: float) -> void:
	if not is_paused():
		if not _can_bridge.is_alive():
			_can_bridge.close_bus()
			_toggle_pause()


func is_paused() -> bool:
	return _is_paused


func close_connection() -> void:
	if not _is_paused:
		_can_bridge.close_bus()
		_toggle_pause()


func connect_interface(interface_name: String) -> bool:
	# Handle CAN bus interaction
	if _is_paused:
		var dbc_load_success: bool = _can_bridge.load_dbc_file(_dbc_line_edit.text)
		if not dbc_load_success:
			return false
		_transmit_table.refresh_dbc()

		var can_up_success: bool = _can_bridge.configure_bus(interface_name)
		if not can_up_success:
			return false
	else:
		return false

	_toggle_pause()
	return true


# Toggle the paused state and update the button text
func _toggle_pause() -> void:
	_is_paused = not _is_paused
	_update_text()
	_status_bar.update_text()
	_update_tab_selectability()
	_update_dbc_editability()


func _update_text() -> void:
	if _is_paused:
		text = "Start"
	else:
		text = "Pause"


# Configuration and conversion tools remain available before connecting.
func _update_tab_selectability() -> void:
	var should_disable_tabs: bool = _is_paused and _receive_table.is_empty()

	for i in range(1, _tab_container.get_child_count()):
		_tab_container.set_tab_disabled(i, should_disable_tabs and i == 1)


func _update_dbc_editability() -> void:
	_dbc_line_edit.editable = _is_paused
	_dbc_open_button.disabled = not _is_paused

	var tooltip_string: String = "" if _is_paused else "Cannot modify DBC file during an active connection"
	_dbc_line_edit.tooltip_text = tooltip_string
	_dbc_open_button.tooltip_text = tooltip_string
