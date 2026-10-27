extends SceneTree


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var scene: Node = load("res://assets/main/main.tscn").instantiate()
	root.add_child(scene)
	current_scene = scene
	var bridge: GodotCanBridge = scene.get_node("GodotCanBridge")
	assert(bridge.load_dbc_file(ProjectSettings.globalize_path("res://tests/transmit.dbc")))
	var table: TransmitTable = scene.get_node("Background/VSplitContainer/TabContainer/Transmit/TransmitTable")
	table.refresh_dbc()
	assert(table.dbc_messages.size() == 1)
	table.add_dbc_send_entry(table.dbc_messages[0])
	var entry: TransmitTable.TransmitTableEntry = table.send_entries[0]
	assert(entry.can_id() == 0x10302 and entry.is_ext_can())
	assert(entry._signal_fields.size() == 3)
	entry._signal_fields[0].text = "0.001"
	entry._signal_fields[1].text = "0x0b"
	entry._signal_fields[2].text = "1"
	var result: Dictionary = JSON.parse_string(bridge.encode_dbc_frame(entry.can_id(), true, '["0.001","0x0b","1"]'))
	assert(PackedByteArray(result.data) == PackedByteArray([111, 18, 131, 58, 11, 1, 0, 0]))
	entry._check_box.button_pressed = true
	assert(not entry._signal_fields[0].editable)
	entry._last_send_time_ms = -100
	entry.send_if_ready()
	entry._check_box.button_pressed = false
	assert(entry._signal_fields[0].editable)
	bridge.load_dbc_file("")
	table.refresh_dbc()
	assert(table.dbc_messages.is_empty())
	assert(entry._check_box.disabled and not entry.sending())
	scene.queue_free()
	await process_frame
	print("DBC transmit integration tests passed")
	quit()
