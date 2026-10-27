class_name RunningInterfaces
extends VBoxContainer

@export var can_bridge: GodotCanBridge
@export var connection: PauseButton
@export var setup: InterfaceControls
@export var setup_dialog: Window
@export var discovery_enabled := true

var _rows: Dictionary = {}
var _list: VBoxContainer
var _empty: Label
var _status: Label
var _elapsed := 0.0


func _ready() -> void:
	setup_dialog.close_requested.connect(setup_dialog.hide)
	setup_dialog.window_input.connect(func(event: InputEvent):
		if event.is_action_pressed("ui_cancel"):
			setup_dialog.hide())
	var heading := HBoxContainer.new()
	add_child(heading)
	var title := Label.new()
	title.text = "CAN interfaces"
	title.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	heading.add_child(title)
	var add := Button.new()
	add.text = "Add interface…"
	add.pressed.connect(func():
		setup_dialog.popup_centered(Vector2i(680, 330))
		if setup._adapters.is_empty():
			setup._driver_changed())
	heading.add_child(add)
	var columns := HBoxContainer.new()
	columns.add_theme_constant_override("separation", 12)
	add_child(columns)
	for column in [["Interface", 120], ["Bitrate", 130], ["Bus load", 80], ["Connection", 110], ["", 70]]:
		var label := Label.new()
		label.text = column[0]
		label.custom_minimum_size.x = column[1]
		if column[0] == "Interface":
			label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		columns.add_child(label)
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	scroll.custom_minimum_size.y = 80
	add_child(scroll)
	_list = VBoxContainer.new()
	_list.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scroll.add_child(_list)
	_empty = Label.new()
	_empty.text = "No running CAN interfaces. Use Add interface… to start one."
	_empty.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_list.add_child(_empty)
	_status = Label.new()
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	add_child(_status)
	setup.interface_started.connect(func(): _status.text = "Starting interface…")


func _process(delta: float) -> void:
	if not discovery_enabled:
		return
	_elapsed += delta
	if _elapsed < 0.5:
		return
	_elapsed = 0.0
	var interfaces: Array = JSON.parse_string(can_bridge.running_interfaces())
	_update_interfaces(interfaces)
	var message: String = can_bridge.interface_operation_status()
	if not message.is_empty():
		_status.text = message
		setup._show_status(message)
		if message.begins_with("ERROR:"):
			setup._pending_selector = ""
		elif message == "Interface operation complete." and OS.get_name() == "Linux" and setup._virtual.button_pressed:
			setup._virtual.button_pressed = false
			setup._refresh_adapters()


func _update_interfaces(interfaces: Array) -> void:
	setup._interfaces = interfaces
	_empty.visible = interfaces.is_empty()
	var names: Array = []
	var connected: String = can_bridge.get_interface() if can_bridge != null else ""
	for interface: Dictionary in interfaces:
		var name: String = interface.name
		if name == connected and interface.state != "up":
			connection.close_connection()
			connected = ""
		names.append(name)
		if not _rows.has(name):
			_add_row(name)
		var row: Dictionary = _rows[name]
		row.name_label.text = setup._preferences.display_name(interface)
		if interface.state != "up":
			row.name_label.text += " (" + str(interface.state).capitalize() + ")"
		row.name_label.tooltip_text = "%s\n%s\n%s: %s\n%s" % [row.name_label.text, name, interface.driver, interface.device, interface.get("error", "")]
		row.bitrate.text = "%s bit/s" % interface.bitrate if interface.bitrate > 0 else "Bitrate: —"
		var load: Variant = interface.get("bus_load")
		row.bus_load.text = "%.2f%%" % float(load) if load != null else "—"
		row.up = interface.state == "up"
		row.stop.disabled = interface.state == "down"
	for name: String in _rows.keys():
		if not names.has(name):
			_rows[name].container.queue_free()
			_rows.erase(name)
	_update_connection_buttons(connected)


func _update_connection_buttons(connected: String) -> void:
	for name: String in _rows:
		var row: Dictionary = _rows[name]
		row.connect.text = "Disconnect" if name == connected else "Connect"
		row.connect.disabled = name != connected and (not connected.is_empty() or not row.up)
		row.connect.tooltip_text = "Disconnect the current interface first." if not connected.is_empty() and name != connected else ""


func _add_row(name: String) -> void:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 12)
	_list.add_child(row)
	var name_label := Label.new()
	name_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	name_label.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
	name_label.custom_minimum_size.x = 120
	row.add_child(name_label)
	var bitrate := Label.new()
	bitrate.custom_minimum_size.x = 130
	row.add_child(bitrate)
	var bus_load := Label.new()
	bus_load.custom_minimum_size.x = 80
	bus_load.tooltip_text = "Estimated bus loading over the last second, including traffic from other clients."
	row.add_child(bus_load)
	var connect := Button.new()
	connect.custom_minimum_size.x = 110
	connect.pressed.connect(_connect_interface.bind(name))
	row.add_child(connect)
	var stop := Button.new()
	stop.text = "Stop"
	stop.custom_minimum_size.x = 70
	stop.pressed.connect(_stop_interface.bind(name))
	row.add_child(stop)
	_rows[name] = {"container": row, "name_label": name_label, "bitrate": bitrate, "bus_load": bus_load, "connect": connect, "stop": stop}


func _connect_interface(name: String) -> void:
	if not connection.is_paused():
		if can_bridge.get_interface() == name:
			connection.close_connection()
	else:
		connection.connect_interface(name)
	_elapsed = 1.0


func _stop_interface(name: String) -> void:
	if can_bridge.get_interface() == name:
		connection.close_connection()
	var error: String = can_bridge.stop_interface(name)
	_status.text = "Stopping interface…" if error.is_empty() else "ERROR: " + error
	if not error.is_empty():
		AlertHandler.display_error(error)
	_elapsed = 1.0
