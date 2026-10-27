extends VBoxContainer

@export var can_bridge: GodotCanBridge
@export var interface_box: LineEdit
@export var pause_button: PauseButton

var _status: Label
var _bitrate: SpinBox
var _virtual: CheckBox
var _driver: OptionButton
var _channel: LineEdit
var _actions: Array[Button] = []


func _ready() -> void:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 8)
	add_child(row)
	var label := Label.new()
	label.text = "Bitrate [bit/s]"
	row.add_child(label)
	_bitrate = SpinBox.new()
	_bitrate.custom_minimum_size.x = 120
	_bitrate.min_value = 1
	_bitrate.max_value = 1000000
	_bitrate.value = 1000000
	row.add_child(_bitrate)
	_virtual = CheckBox.new()
	_virtual.text = "Virtual CAN"
	_virtual.visible = OS.get_name() == "Linux"
	row.add_child(_virtual)
	_driver = OptionButton.new()
	for driver in ["gsusb", "slcan", "pcan"]:
		_driver.add_item(driver)
	_driver.visible = OS.get_name() == "Windows"
	row.add_child(_driver)
	_channel = LineEdit.new()
	_channel.placeholder_text = "Adapter / channel (auto)"
	_channel.visible = OS.get_name() == "Windows"
	_channel.custom_minimum_size.x = 200
	row.add_child(_channel)
	var actions := HBoxContainer.new()
	actions.add_theme_constant_override("separation", 12)
	add_child(actions)
	_add_action(actions, "Bring up / Restart", func(): _manage(false))
	if OS.get_name() == "Linux":
		_add_action(actions, "Create vcan", func():
			_virtual.button_pressed = true
			_manage(true))
	if OS.get_name() == "Windows":
		_add_action(actions, "Stop managed server", func():
			var error: String = can_bridge.stop_managed_server()
			_show_status("Managed server stopped" if error.is_empty() else "ERROR: " + error))
	_status = Label.new()
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_status.text = "Manage the interface here, then click Start to connect."
	add_child(_status)


func _add_action(row: HBoxContainer, label: String, action: Callable) -> void:
	var button := Button.new()
	button.text = label
	button.custom_minimum_size = Vector2(160, 32)
	button.pressed.connect(action)
	row.add_child(button)
	_actions.append(button)


func _process(_delta: float) -> void:
	for button in _actions:
		button.disabled = not pause_button.is_paused()
	var result: String = can_bridge.interface_operation_status()
	if not result.is_empty():
		_show_status(result)


func _manage(create: bool) -> void:
	var error: String = can_bridge.manage_interface(interface_box.text.strip_edges(), int(_bitrate.value), _virtual.button_pressed, create, _driver.get_item_text(_driver.selected), _channel.text, ProjectSettings.globalize_path("user://canserver.log"))
	_show_status("Starting interface… Click Start once it is ready." if error.is_empty() else "ERROR: " + error)


func _show_status(message: String) -> void:
	_status.text = message
	if message.begins_with("ERROR:"):
		AlertHandler.display_error(message)
