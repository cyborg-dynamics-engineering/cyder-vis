extends Window
class_name Mf4Viewer

var _decoder := Mf4Decoder.new()
var _open: Button
var _dbc: Button
var _clear_dbc: Button
var _file: Label
var _dbc_label: Label
var _summary: Label
var _groups: OptionButton
var _table: Tree
var _previous: Button
var _next: Button
var _page_number: SpinBox
var _range: Label
var _status: Label
var _info: Dictionary = {}
var _path: String = ""
var _offset: int = 0
var _page_size: int = 500
var _busy: bool = false
var _updating_page: bool = false
var _last_directory: String = ""
var _rows: Array = []


func _ready() -> void:
	title = "MF4 capture viewer"
	min_size = Vector2i(680, 420)
	size = Vector2i(1100, 680)
	transient = true
	close_requested.connect(queue_free)
	_last_directory = DbcFileButton.home_directory()
	var margin := MarginContainer.new()
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side in ["left", "right", "top", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 12)
	add_child(margin)
	var layout := VBoxContainer.new()
	layout.add_theme_constant_override("separation", 8)
	margin.add_child(layout)
	var actions := HFlowContainer.new()
	layout.add_child(actions)
	_open = _button(actions, "Open MF4…", _choose_capture)
	_dbc = _button(actions, "Load DBC…", _choose_dbc)
	_dbc.tooltip_text = "Decode raw CAN frames using a DBC. Recorded signal files already contain their values."
	_clear_dbc = _button(actions, "Clear DBC", _remove_dbc)
	_dbc_label = _label(actions, "No DBC loaded")
	_file = _label(layout, "Open a saved MF4 capture to browse its frames or signals.")
	_file.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
	_summary = _label(layout, "")
	_groups = OptionButton.new()
	_groups.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_groups.item_selected.connect(_select_group)
	layout.add_child(_groups)
	_table = Tree.new()
	_table.hide_root = true
	_table.column_titles_visible = true
	_table.select_mode = Tree.SELECT_ROW
	_table.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_table.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	layout.add_child(_table)
	var navigation := HFlowContainer.new()
	layout.add_child(navigation)
	_previous = _button(navigation, "Previous", _previous_page)
	_next = _button(navigation, "Next", _next_page)
	_label(navigation, "Page")
	_page_number = SpinBox.new()
	_page_number.min_value = 1
	_page_number.max_value = 1
	_page_number.step = 1
	_page_number.custom_minimum_size.x = 90
	_page_number.value_changed.connect(_jump_page)
	navigation.add_child(_page_number)
	_range = _label(navigation, "")
	_button(navigation, "Copy row", _copy_row)
	_status = _label(layout, "")
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_update_controls()


func _exit_tree() -> void:
	_decoder.close_capture()


func _process(_delta: float) -> void:
	var reply := _decoder.poll()
	if reply.is_empty():
		return
	_busy = false
	var result: Dictionary = JSON.parse_string(reply)
	match result.get("kind", ""):
		"opened":
			_path = result.path
			_info = result.info
			_last_directory = _path.get_base_dir()
			_file.text = _path.get_file()
			_file.tooltip_text = _path
			title = "MF4 — " + _path.get_file()
			var start_seconds := int(_info.start_time_ns) / 1000000000
			_summary.text = "MDF %s · %d %s · Started %s UTC" % [_info.version, _info.groups.size(), "group" if _info.groups.size() == 1 else "groups", Time.get_datetime_string_from_unix_time(start_seconds, true)]
			_groups.clear()
			for group: Dictionary in _info.groups:
				_groups.add_item("%s · %s · %d rows" % [group.name, group.description, group.records])
			_table.clear()
			_rows.clear()
			_offset = 0
			if _info.groups.is_empty():
				_range.text = "0 rows"
				_status.text = "This capture contains no recorded samples."
			else:
				_groups.select(0)
				_select_group(0)
		"page":
			_show_page(result.page)
		"dbc":
			_clear_dbc.disabled = str(result.path).is_empty()
			_dbc_label.text = "No DBC loaded" if str(result.path).is_empty() else str(result.path).get_file()
			_dbc_label.tooltip_text = result.path
			if not _info.get("groups", []).is_empty():
				_request_page(_offset)
			else:
				_status.text = "DBC updated."
		"error":
			_status.text = result.get("error", "Could not read MF4 capture.")
			_updating_page = true
			_page_number.value = 1 + _offset / _page_size
			_updating_page = false
	_update_controls()


func open_file(path: String) -> void:
	if _busy:
		return
	_begin(_decoder.open_capture(path), "Opening MF4 capture…")


func create_file_dialog() -> FileDialog:
	var dialog := FileDialog.new()
	dialog.file_mode = FileDialog.FILE_MODE_OPEN_FILE
	dialog.access = FileDialog.ACCESS_FILESYSTEM
	dialog.current_dir = _last_directory
	dialog.filters = ["*.mf4; MDF 4 captures"]
	dialog.file_selected.connect(open_file)
	dialog.file_selected.connect(func(_path_selected: String): dialog.queue_free())
	dialog.canceled.connect(dialog.queue_free)
	return dialog


func _choose_capture() -> void:
	var dialog := create_file_dialog()
	add_child(dialog)
	dialog.popup_centered_ratio(0.8)


func _choose_dbc() -> void:
	var dialog := FileDialog.new()
	dialog.file_mode = FileDialog.FILE_MODE_OPEN_FILE
	dialog.access = FileDialog.ACCESS_FILESYSTEM
	dialog.current_dir = _last_directory
	dialog.filters = ["*.dbc; CAN DBC files"]
	add_child(dialog)
	dialog.file_selected.connect(func(path: String):
		_set_dbc(path)
		dialog.queue_free())
	dialog.canceled.connect(dialog.queue_free)
	dialog.popup_centered_ratio(0.8)


func _set_dbc(path: String) -> void:
	_begin(_decoder.set_dbc(path), "Loading DBC…")


func _remove_dbc() -> void:
	_set_dbc("")


func _select_group(index: int) -> void:
	var group: Dictionary = _info.groups[index]
	_rows.clear()
	_table.clear()
	_offset = 0
	_table.columns = group.columns.size()
	for column in group.columns.size():
		_table.set_column_title(column, group.columns[column])
		_table.set_column_custom_minimum_width(column, 120 if column < 5 else 200)
		_table.set_column_expand(column, true)
	# Bound the number of visible cells for files with many signal channels.
	_page_size = mini(500, maxi(1, 8000 / group.columns.size()))
	_updating_page = true
	_page_number.max_value = maxi(1, ceili(float(group.records) / _page_size))
	_page_number.value = 1
	_updating_page = false
	_request_page(0)


func _request_page(offset: int) -> void:
	if _busy or _groups.selected < 0:
		return
	_begin(_decoder.request_page(_groups.selected, offset, _page_size), "Reading capture…")


func _show_page(page: Dictionary) -> void:
	_offset = page.offset
	_rows = page.rows
	_table.clear()
	var tree_root := _table.create_item()
	for row: Array in _rows:
		var item := _table.create_item(tree_root)
		for column in row.size():
			item.set_text(column, str(row[column]))
			item.set_tooltip_text(column, str(row[column]))
	_updating_page = true
	_page_number.value = 1 + _offset / _page_size
	_updating_page = false
	_range.text = "Rows %d–%d of %d" % [0 if _rows.is_empty() else _offset + 1, _offset + _rows.size(), page.total]
	_status.text = "This group contains no recorded samples." if _rows.is_empty() else "Read-only capture · Times are relative to the file's start time."
	_update_controls()


func _previous_page() -> void:
	_request_page(maxi(0, _offset - _page_size))


func _next_page() -> void:
	_request_page(_offset + _page_size)


func _jump_page(number: float) -> void:
	if not _updating_page:
		_request_page((int(number) - 1) * _page_size)


func _copy_row() -> void:
	var selected := _table.get_selected()
	if selected == null:
		return
	var fields := PackedStringArray()
	for column in _table.columns:
		fields.append(selected.get_text(column))
	DisplayServer.clipboard_set("\t".join(fields))


func _begin(error: String, status: String) -> void:
	_busy = error.is_empty()
	_status.text = status if error.is_empty() else error
	_update_controls()


func _update_controls() -> void:
	_open.disabled = _busy
	_dbc.disabled = _busy
	_clear_dbc.disabled = _busy or _dbc_label.tooltip_text.is_empty()
	var empty: bool = _info.get("groups", []).is_empty()
	_groups.disabled = _busy or empty
	_page_number.editable = not _busy and not empty
	_previous.disabled = _busy or empty or _offset == 0
	_next.disabled = _busy or empty
	if not empty:
		_next.disabled = _busy or _offset + _rows.size() >= int(_info.groups[_groups.selected].records)


func _button(parent: Node, label: String, callback: Callable) -> Button:
	var button := Button.new()
	button.text = label
	button.pressed.connect(callback)
	parent.add_child(button)
	return button


func _label(parent: Node, text_value: String) -> Label:
	var label := Label.new()
	label.text = text_value
	parent.add_child(label)
	return label
