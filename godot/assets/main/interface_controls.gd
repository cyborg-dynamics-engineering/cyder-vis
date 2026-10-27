class_name InterfaceControls
extends VBoxContainer

@export var can_bridge: GodotCanBridge
@export var discovery_enabled := true

signal interface_started

var _status: Label
var _bitrate: SpinBox
var _virtual: CheckBox
var _driver: OptionButton
var _adapter: OptionButton
var _refresh: Button
var _nickname_row: HBoxContainer
var _nickname: LineEdit
var _save_name: Button
var _identity_note: Label
var _actions: Array[Button] = []
var _preferences := AdapterPreferences.new()
var _adapters: Array = []
var _scan_running := false
var _scan_pending := false
var _windows := OS.get_name() == "Windows"
var _interfaces: Array = []
var _pending_selector := ""
var _pending_since := 0


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
	_virtual.text = "Create virtual CAN"
	_virtual.visible = OS.get_name() == "Linux"
	row.add_child(_virtual)
	_driver = OptionButton.new()
	for driver in ["gsusb", "slcan", "pcan"]:
		_driver.add_item(driver)
	_driver.visible = _windows
	_driver.item_selected.connect(func(_index: int): _driver_changed())
	row.add_child(_driver)
	var adapter_row := HBoxContainer.new()
	add_child(adapter_row)
	_adapter = OptionButton.new()
	_adapter.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_adapter.fit_to_longest_item = false
	_adapter.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
	_adapter.item_selected.connect(_adapter_selected)
	adapter_row.add_child(_adapter)
	_refresh = Button.new()
	_refresh.text = "Refresh"
	_refresh.pressed.connect(_refresh_adapters)
	adapter_row.add_child(_refresh)
	_nickname_row = HBoxContainer.new()
	_nickname_row.visible = _windows
	add_child(_nickname_row)
	_nickname = LineEdit.new()
	_nickname.placeholder_text = "Adapter nickname"
	_nickname.max_length = 80
	_nickname.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_nickname.text_submitted.connect(func(_value: String): _save_nickname())
	_nickname_row.add_child(_nickname)
	_save_name = Button.new()
	_save_name.text = "Save name"
	_save_name.pressed.connect(_save_nickname)
	_nickname_row.add_child(_save_name)
	_identity_note = Label.new()
	_identity_note.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_identity_note.visible = _windows
	add_child(_identity_note)
	var actions := HFlowContainer.new()
	actions.add_theme_constant_override("h_separation", 12)
	add_child(actions)
	_add_action(actions, "Start", func(): _manage(false))
	_status = Label.new()
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_status.text = "Choose an adapter and start its CAN interface."
	add_child(_status)
	var error := _preferences.load_preferences()
	if error != OK:
		_show_status("ERROR: Cannot load adapter preferences: " + error_string(error))
	if is_visible_in_tree():
		_driver_changed()


func _driver_name() -> String:
	return _driver.get_item_text(_driver.selected) if _windows else "socketcan"


func _driver_changed() -> void:
	_adapters.clear()
	_adapter.clear()
	_adapter.add_item("Scanning adapters…")
	_nickname.text = ""
	_identity_note.text = ""
	_nickname_row.visible = _windows and _driver_name() != "slcan"
	_refresh_adapters()


func _refresh_adapters() -> void:
	if not discovery_enabled:
		return
	if _scan_running:
		_scan_pending = true
		return
	var error: String = can_bridge.scan_adapters(_driver_name())
	if not error.is_empty():
		_show_status("ERROR: " + error)
		return
	_scan_running = true
	_refresh.text = "Scanning…"


func _add_action(row: Container, label: String, action: Callable) -> void:
	var button := Button.new()
	button.text = label
	button.custom_minimum_size = Vector2(160, 32)
	button.pressed.connect(action)
	row.add_child(button)
	_actions.append(button)


func _process(_delta: float) -> void:
	var adapter := _selection()
	var already_up: bool = _running_match().get("state", "") == "up"
	var pending: bool = not _pending_selector.is_empty() and _pending_selector == adapter.get("selector", "")
	if already_up or Time.get_ticks_msec() - _pending_since > 15000:
		_pending_selector = ""
	var create_virtual := not _windows and _virtual.button_pressed
	_actions[0].disabled = already_up or pending or _scan_running or (not create_virtual and not adapter.get("available", false))
	_actions[0].tooltip_text = "The interface is already up." if already_up else ("Starting interface…" if pending else "")
	_adapter.disabled = _scan_running or _adapters.is_empty() or create_virtual
	_refresh.disabled = _scan_running
	var can_name: bool = not str(adapter.get("identity", "")).is_empty() and not _scan_running
	_nickname.editable = can_name
	_save_name.disabled = not can_name
	if can_bridge == null:
		return
	if _scan_running:
		var scan: String = can_bridge.poll_adapter_scan()
		if not scan.is_empty():
			_scan_running = false
			_refresh.text = "Refresh"
			var parsed: Variant = JSON.parse_string(scan)
			if parsed is Dictionary:
				_accept_scan(parsed)
			if _scan_pending:
				_scan_pending = false
				_refresh_adapters()


func _accept_scan(result: Dictionary) -> void:
	if result.get("driver", _driver_name()) != _driver_name():
		return # Ignore results from a driver the user has since deselected.
	if result.has("error"):
		_adapters.clear()
		_adapter.clear()
		_adapter.add_item("Adapter discovery failed — see below")
		_nickname.text = ""
		_identity_note.text = ""
		_show_status(str(result.error))
		return
	var previous: String = _selection().get("identity", "")
	if previous.is_empty():
		previous = _preferences.selected(_driver_name())
	_adapters = result.get("adapters", [])
	_adapter.clear()
	var placeholder := "No COM ports found" if _driver_name() == "slcan" else "No adapters found"
	if not _adapters.is_empty():
		placeholder = "Choose a COM port…" if _driver_name() == "slcan" else "Choose an adapter…"
	_adapter.add_item(placeholder)
	_adapter.set_item_disabled(0, true)
	var selected_index := 0
	var available_count := 0
	var only_available := 0
	for i in _adapters.size():
		var adapter: Dictionary = _adapters[i]
		_adapter.add_item(_adapter_label(adapter))
		_adapter.set_item_tooltip(i + 1, adapter.label + "\n" + adapter.get("reason", ""))
		# Busy entries remain selectable for renaming; Bring up stays disabled.
		if adapter.get("available", false):
			available_count += 1
			only_available = i + 1
		if not previous.is_empty() and adapter.get("identity", "") == previous:
			selected_index = i + 1
	if selected_index == 0 and previous.is_empty() and available_count == 1:
		selected_index = only_available
	if selected_index == 0 and not previous.is_empty():
		_adapter.set_item_text(0, "Saved adapter unavailable — choose an adapter…")
	_adapter.select(selected_index)
	_update_nickname()
	_status.text = "Choose an adapter and start its CAN interface."


func _selection() -> Dictionary:
	var index := _adapter.selected - 1
	return _adapters[index] if index >= 0 and index < _adapters.size() else {}


func _adapter_label(adapter: Dictionary) -> String:
	var nickname := _preferences.nickname(adapter.get("identity", ""))
	var label: String = adapter.label
	if not nickname.is_empty():
		label = nickname + " — " + label
	if not adapter.get("available", false):
		label += " [" + adapter.get("reason", "Unavailable") + "]"
	return label


func _adapter_selected(_index: int) -> void:
	_update_nickname()
	var identity: String = _selection().get("identity", "")
	if not identity.is_empty():
		var error := _preferences.save_selection(_driver_name(), identity)
		if error != OK:
			_show_status("ERROR: Cannot save adapter selection: " + error_string(error))


func _update_nickname() -> void:
	var adapter := _selection()
	_nickname.text = _preferences.nickname(adapter.get("identity", ""))
	_identity_note.text = adapter.get("identity_note", "")
	_adapter.tooltip_text = adapter.get("label", "") + "\n" + adapter.get("reason", "")


func _save_nickname() -> void:
	var identity: String = _selection().get("identity", "")
	if identity.is_empty():
		return
	var error := _preferences.save_nickname(identity, _nickname.text)
	if error != OK:
		_show_status("ERROR: Cannot save adapter nickname: " + error_string(error))
		return
	_nickname.text = _preferences.nickname(identity)
	for i in _adapters.size():
		_adapter.set_item_text(i + 1, _adapter_label(_adapters[i]))
	_status.text = "Adapter nickname saved. Clear the name and save to remove it."


func _manage(create: bool) -> void:
	var adapter := _selection()
	var selector: String = adapter.get("selector", "")
	var create_virtual := not _windows and _virtual.button_pressed
	if not create_virtual and (selector.is_empty() or not adapter.get("available", false) or _scan_running):
		_show_status("ERROR: Select an available adapter first.")
		return
	if _running_match().get("state", "") == "up":
		return
	_adapter_selected(_adapter.selected)
	var identity: String = adapter.get("identity", "")
	var device_name: String = adapter.get("device_name", adapter.get("label", selector))
	var display_name := _preferences.nickname(identity)
	if display_name.is_empty():
		display_name = device_name
	var name := selector
	if _windows:
		name = can_bridge.interface_service_name(display_name, identity)
	elif create_virtual:
		var index := 0
		while DirAccess.dir_exists_absolute("/sys/class/net/vcan%d" % index):
			index += 1
		name = "vcan%d" % index
		selector = name
		device_name = name
	var existing := _running_match()
	var error: String
	if _windows and not existing.is_empty():
		name = existing.name
		error = can_bridge.resume_interface(name, int(_bitrate.value))
	else:
		error = can_bridge.manage_interface(name, int(_bitrate.value), create_virtual or selector.begins_with("vcan"), create or create_virtual, _driver_name(), selector, ProjectSettings.globalize_path("user://canserver_%s.log" % name))
	if error.is_empty():
		_pending_selector = selector
		_pending_since = Time.get_ticks_msec()
		if _windows:
			var saved := _preferences.record_interface(name, identity, device_name, _driver_name(), selector, adapter.get("label", device_name))
			if saved != OK:
				_show_status("ERROR: Interface started, but its display name could not be saved: " + error_string(saved))
		interface_started.emit()
	_show_status("Starting interface… It will appear in the running-interface list." if error.is_empty() else "ERROR: " + error)


func _running_match() -> Dictionary:
	var selector: String = _selection().get("selector", "")
	for interface: Dictionary in _interfaces:
		if interface.driver == _driver_name() and interface.device == selector:
			return interface
	return {}


func _show_status(message: String) -> void:
	_status.text = message
	if message.begins_with("ERROR:"):
		AlertHandler.display_error(message)
