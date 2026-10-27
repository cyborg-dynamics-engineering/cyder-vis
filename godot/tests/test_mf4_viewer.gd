extends SceneTree


func _initialize() -> void:
	Engine.max_fps = 60
	_run.call_deferred()


func _idle(viewer: Mf4Viewer) -> void:
	for attempt in 100:
		await process_frame
		if not viewer._busy:
			return
	assert(false, "MF4 viewer did not finish its background request")


func _run() -> void:
	var viewer := Mf4Viewer.new()
	root.add_child(viewer)
	var dialog := viewer.create_file_dialog()
	assert(dialog.file_mode == FileDialog.FILE_MODE_OPEN_FILE)
	assert(dialog.access == FileDialog.ACCESS_FILESYSTEM)
	assert(dialog.current_dir.replace("\\", "/") == DbcFileButton.home_directory())
	assert(dialog.filters[0].begins_with("*.mf4"))
	dialog.free()
	assert(viewer._next.disabled and viewer._previous.disabled)
	assert(not viewer._open.disabled)
	var raw := ProjectSettings.globalize_path("res://tests/mf4/raw.mf4")
	viewer.open_file(raw)
	assert(viewer._busy and viewer._open.disabled)
	await _idle(viewer)
	assert(viewer._info.groups.size() == 3, viewer._status.text)
	assert(viewer._rows.size() == 500, viewer._status.text)
	assert(viewer._rows[0][2] == "10302")
	assert(viewer._rows[0][3].contains("EXT"))
	assert(viewer._rows[0][5] == "6F 12 83 3A 0B 01 00 00")
	assert(viewer._rows[0][6].is_empty())
	assert(viewer._table.columns == 7)
	viewer._next_page()
	await _idle(viewer)
	assert(viewer._offset == 500 and viewer._page_number.value == 2)
	viewer._page_number.value = 5
	await _idle(viewer)
	assert(viewer._offset == 2000 and viewer._rows.size() == 52)
	assert(viewer._rows[51][3].contains("FD BRS ESI"))
	assert(viewer._next.disabled and not viewer._previous.disabled)
	viewer._previous_page()
	await _idle(viewer)
	assert(viewer._offset == 1500)
	viewer._set_dbc(ProjectSettings.globalize_path("res://tests/transmit.dbc"))
	await _idle(viewer)
	assert(viewer._rows[0][6].contains("SetComplementaryFilter.odr = 11"))
	assert(not viewer._clear_dbc.disabled)
	viewer._set_dbc(raw)
	await _idle(viewer)
	assert(viewer._status.text.begins_with("Cannot load DBC"))
	assert(viewer._dbc_label.text == "transmit.dbc")
	viewer.open_file(ProjectSettings.globalize_path("res://tests/transmit.dbc"))
	await _idle(viewer)
	assert(viewer._status.text.contains("MDF 4"), viewer._status.text)
	assert(viewer._path == raw)
	viewer._next_page()
	await _idle(viewer)
	assert(viewer._rows[0][6].contains("SetComplementaryFilter.odr = 11"))
	for group in [1, 2]:
		viewer._groups.select(group)
		viewer._groups.item_selected.emit(group)
		await _idle(viewer)
		assert(viewer._rows.size() == 1)
		assert(viewer._rows[0][3].contains("ERR" if group == 1 else "RTR"))
	viewer.open_file(ProjectSettings.globalize_path("res://tests/mf4/signals.mf4"))
	await _idle(viewer)
	assert(viewer._table.columns == 5)
	assert(viewer._rows[0][1] == "-40")
	assert(viewer._rows[0][2] == "-1234")
	assert(viewer._rows[0][4] == "18446744073709551615")
	assert(viewer._rows[10][3] == "N/A")
	assert(viewer._table.get_column_title(2) == "Motor.temperature (°C)")
	viewer._remove_dbc()
	await _idle(viewer)
	assert(viewer._clear_dbc.disabled)
	viewer.open_file(ProjectSettings.globalize_path("res://tests/mf4/empty.mf4"))
	await _idle(viewer)
	assert(viewer._info.groups.is_empty())
	assert(viewer._rows.is_empty())
	assert(viewer._status.text.contains("no recorded samples"))
	assert(viewer._groups.disabled and viewer._next.disabled and viewer._previous.disabled)
	viewer.queue_free()
	await process_frame
	print("MF4 viewer open, paging, DBC, signals, empty files and recovery tests passed")
	quit()
