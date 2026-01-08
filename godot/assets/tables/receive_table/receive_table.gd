extends Control
class_name ReceiveTable


@export_category("Node References")
@export var godot_can_bridge: GodotCanBridge
@export var pause_button: PauseButton
@export var can_graph: CanGraph
@export var can_id_format_button: CanFormatButton
@export var can_data_format_button: CanFormatButton
@export var time_format_button: CanFormatButton
@export var context_menu: ContextMenu

@onready var table_row = preload("res://assets/tables/table_row.tscn")
@onready var table_cell = preload("res://assets/tables/table_cell.tscn")
@onready var table_button = preload("res://assets/tables/table_button.tscn")
@onready var rows: Control = get_node("Rows")
@onready var existing_can_entries: Dictionary[int, ReceiveTableEntry] = {}

const TIMESTAMP_IDX = 0
const FREQUENCY_IDX = 1
const CAN_ID_IDX = 2
const MSG_NAME_IDX = 3
const DATA_START_IDX = 4
const IS_EXTENDED_IDX = -1

const CELL_HEIGHT = 25
const CELL_WIDTHS = [100, 80, 100, 100, 80]


func _ready() -> void:
	_generate_header_row()


func _process(_delta: float) -> void:
	if not pause_button.is_paused():
		render(godot_can_bridge.get_can_table())


func _unhandled_input(event: InputEvent) -> void:
	# Produce a 'clearing' context menu whenever right clicks fall on the receive table
	if event is InputEventMouseButton and event.is_pressed() and event.button_index == MOUSE_BUTTON_RIGHT:
		# Clear all old context menu options
		context_menu.clear_items()

		# Check if the mouse is over any specific row entries and if so add the 'clear msg' item
		var last_mouse_pos = get_global_mouse_position()
		for entry: ReceiveTableEntry in existing_can_entries.values():
			var control_rect: Rect2 = entry.get_row().get_global_rect()
			if control_rect.has_point(last_mouse_pos):
				context_menu.add_item("Clear: " + entry.formatted_can_id(), clear_row.bind(entry.id()))
				continue
		
		# Add 'clear all' item
		context_menu.add_item("Clear all", clear_all)

		context_menu.appear(last_mouse_pos)


# Updates the rows in the table with incoming data
func render(data: Array) -> void:
	# Do nothing if no data received
	if not data:
		return

	# Update current timestamp for realtime plot
	can_graph.update_current_time(_get_largest_timestamp(data))

	# For each data item
	for data_entry: Array in data:
		var can_id := int(data_entry[CAN_ID_IDX])

		# If a new CAN_ID, create new row
		var table_entry: ReceiveTableEntry = existing_can_entries.get(can_id)
		if table_entry == null:
			existing_can_entries[can_id] = ReceiveTableEntry.new(self, data_entry)
			sort_entries()

		# Else, we have already discovered this CAN_ID
		else:
			table_entry.update(data_entry)


# Clears all rows from the table
func clear_all() -> void:
	# Clear rows and entries from Godot side
	for entry: ReceiveTableEntry in existing_can_entries.values():
		entry.get_row().queue_free()
	existing_can_entries.clear()

	# Clear entries from rust side
	godot_can_bridge.clear_can_table()


# Clears a specific row from the table
func clear_row(can_id: int) -> void:
	if not existing_can_entries.has(can_id):
		printerr("Attempted to clear a CAN ID that doesn't exist in the receive table: " + str(can_id))
		return

	# Clear rows and entries from Godot side
	existing_can_entries[can_id].get_row().queue_free()
	existing_can_entries.erase(can_id)

	# Clear entry from rust side
	godot_can_bridge.clear_can_entry(can_id)


# Re-renders every CAN entry. Useful for updating the table on formatting state changes.
func update_formatting() -> void:
	# Update the formatting of each CAN entry
	for entry: ReceiveTableEntry in existing_can_entries.values():
		entry.update_labels()
	
	# Update the frequency header formatting
	rows.get_child(0).get_child(FREQUENCY_IDX).get_node("Label").text = "FREQ [Hz]" if time_format_button.format_on() else "T [ms]"


# Converts a microsecond system timestamp to seconds from start of program
func timestamp_to_s(timestamp: String) -> float:
	return int(timestamp) * 1e-6


# Returns the most recent timestamp from an array of incoming CAN messages
func _get_largest_timestamp(data: Array) -> float:
	var largest_stamp_s: float = 0.0

	for entry: Array in data:
		var entry_timestamp_s := timestamp_to_s(entry[TIMESTAMP_IDX])
		if entry_timestamp_s > largest_stamp_s:
			largest_stamp_s = entry_timestamp_s
	
	return largest_stamp_s


# Adds the header row to the table, should only be called once
func _generate_header_row() -> void:
	var header_row: BoxContainer = table_row.instantiate()
	rows.add_child(header_row)

	const HEADER_LABELS = ["TIMESTAMP", "FREQ [Hz]", "CAN ID", "MSG NAME", "DATA"]
	for i in range(len(HEADER_LABELS)):
		var cell: PanelContainer = table_cell.instantiate()
		cell.custom_minimum_size = Vector2(CELL_WIDTHS[i], CELL_HEIGHT)
		if i == DATA_START_IDX:
			cell.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		_update_label_and_font_size(cell.get_node("Label"), str(HEADER_LABELS[i]), CELL_WIDTHS[i])
		cell.get_node("Label").horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		header_row.add_child(cell)


# Updates the text and tooltip of a label, making adjustments to size and concatonating the label if neccesary
static func _update_label_and_font_size(label, new_text: String, width: int) -> void:
	# Use the full text as a tooltip (mouse hover)
	label.tooltip_text = new_text

	# Truncate string if needed to fit within the allowed width.
	const PADDING_PX: int = 10
	const FONT_SIZE: int = 14
	var truncated_text := new_text
	while ThemeDB.fallback_font.get_string_size(truncated_text, HORIZONTAL_ALIGNMENT_CENTER, -1, FONT_SIZE).x > (width - PADDING_PX):
			truncated_text = truncated_text.left(-1)

	label.text = truncated_text


# Sorts the row nodes in the table to be in order of lowest to highest CAN ID
func sort_entries() -> void:
	var can_ids = existing_can_entries.keys()
	can_ids.sort()

	for i in range(len(can_ids)):
		rows.move_child(existing_can_entries.get(can_ids[i]).get_row(), i + 1)


# Returns true if there are no CAN messages in the receive table history
func is_empty() -> bool:
	return existing_can_entries.is_empty()


class ReceiveTableEntry:
	var _last_receive_time_s: float
	var _frequency_hz: float
	var _can_id: int
	var _msg_name: String
	var _is_extended: bool
	var _data: Array[String]
	var _row: Node
	var _receive_table: ReceiveTable


	func _init(receive_table: ReceiveTable, frame: Array) -> void:
		_receive_table = receive_table
		_last_receive_time_s = -1.0
		update(frame)


	# Instantiates the table cell nodes holding the labels into this entry's row
	func _instantiate_labels(frame: Array):
		_row = _receive_table.table_row.instantiate()
		_receive_table.rows.add_child(_row)

		if self.is_deserialised():
			# For deserialised data we need to add buttons to enable logging
			for i in len(frame) - 1:
				var cell_width = CELL_WIDTHS[i] if i < DATA_START_IDX else CELL_WIDTHS[DATA_START_IDX]
				var cell_size = Vector2(cell_width, CELL_HEIGHT)

				var cell: Node
				var is_button = (i >= DATA_START_IDX) and (i % 2 == 0)
				if is_button:
					cell = _receive_table.table_button.instantiate()
					cell.pressed.connect(_receive_table.can_graph.toggle_plot_element.bind(self, str(frame[i])))
				else:
					cell = _receive_table.table_cell.instantiate()

				cell.custom_minimum_size = cell_size
				
				_row.add_child(cell)

		else:
			# For unknown data we just print the raw bytes as labels
			for i in len(frame) - 1:
				var cell_width = CELL_WIDTHS[i] if i < DATA_START_IDX else CELL_WIDTHS[DATA_START_IDX]
				var cell_size = Vector2(cell_width, CELL_HEIGHT)

				var cell: PanelContainer = _receive_table.table_cell.instantiate()
				cell.custom_minimum_size = cell_size
				
				_row.add_child(cell)
		
		# Set right alignment for specific cells
		_row.get_child(TIMESTAMP_IDX).get_node("Label").horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
		_row.get_child(FREQUENCY_IDX).get_node("Label").horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
		_row.get_child(CAN_ID_IDX).get_node("Label").horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT

		# Apply the correct label text and formatting to each cell
		update_labels()


	func update(new_frame: Array) -> void:
		var new_timestamp_s = _receive_table.timestamp_to_s(new_frame[TIMESTAMP_IDX])

		# Don't bother updating if we haven't received a more recent message
		if new_timestamp_s <= _last_receive_time_s:
			return

		var prev_data_size: int = _data.size()

		_last_receive_time_s = new_timestamp_s
		_frequency_hz = float(new_frame[FREQUENCY_IDX])
		_can_id = int(new_frame[CAN_ID_IDX])

		_msg_name = new_frame[MSG_NAME_IDX]
		_is_extended = new_frame[IS_EXTENDED_IDX].to_lower() == "true"
		_data = []
		for i in range(DATA_START_IDX, len(new_frame) - 1):
			_data.append(new_frame[i])

		# Always make sure a row exists before updating labels
		if not is_instance_valid(_row) or prev_data_size != _data.size():
			if is_instance_valid(_row):
				_row.queue_free()
			_instantiate_labels(new_frame)
		else:
			update_labels()


	func id() -> int:
		return _can_id


	func is_deserialised() -> bool:
		return not _msg_name.is_empty()


	func is_ext_can() -> bool:
		return _is_extended


	func get_row() -> Node:
		return _row


	func update_labels() -> void:
		var entry_row_cells := _row.get_children()

		ReceiveTable._update_label_and_font_size(entry_row_cells[TIMESTAMP_IDX].get_node("Label"), "%.3f" % _last_receive_time_s, CELL_WIDTHS[TIMESTAMP_IDX])
		ReceiveTable._update_label_and_font_size(entry_row_cells[FREQUENCY_IDX].get_node("Label"), _formatted_frequency(), CELL_WIDTHS[FREQUENCY_IDX])
		ReceiveTable._update_label_and_font_size(entry_row_cells[CAN_ID_IDX].get_node("Label"), formatted_can_id(), CELL_WIDTHS[CAN_ID_IDX])
		ReceiveTable._update_label_and_font_size(entry_row_cells[MSG_NAME_IDX].get_node("Label"), _msg_name, CELL_WIDTHS[MSG_NAME_IDX])
	
		# If the payload is empty, then display an empty string
		if len(_data) == 1 and _data[0] == "":
			ReceiveTable._update_label_and_font_size(entry_row_cells[DATA_START_IDX].get_node("Label"), "", CELL_WIDTHS[DATA_START_IDX])
			return

		for i in range(len(_data)):
			# If this data has been deserialised, format the button cells as strings instead of data bytes
			if self.is_deserialised():
				var is_button = (i % 2 == 0)
				if is_button:
					ReceiveTable._update_label_and_font_size(entry_row_cells[DATA_START_IDX + i], _data[i], CELL_WIDTHS[DATA_START_IDX])
				else:
					ReceiveTable._update_label_and_font_size(entry_row_cells[DATA_START_IDX + i].get_node("Label"), _data[i], CELL_WIDTHS[DATA_START_IDX])

					# If the can graph is plotting this data point, forward it to the graph
					var label: String = _data[i - 1]
					if _receive_table.can_graph.has_plot_element(self, label):
						_receive_table.can_graph.add_data_point(self, label, _last_receive_time_s, float(_data[i].trim_suffix("*")))
			else:
				# For regular labels, update with CAN byte formatting
				ReceiveTable._update_label_and_font_size(entry_row_cells[DATA_START_IDX + i].get_node("Label"), _format_can_data_byte(int(_data[i])), CELL_WIDTHS[DATA_START_IDX])


	func formatted_can_id() -> String:
		# Assumes 31 bit length
		if _receive_table.can_id_format_button.format_on():
			return "0x" + ("%08x" % _can_id).to_upper() if is_ext_can() else "0x" + ("%03x" % _can_id).to_upper()
		else:
			return "0d" + ("%09d" % _can_id) if is_ext_can() else "0d" + ("%04d" % _can_id)


	func _format_can_data_byte(byte: int) -> String:
		# Assumes 8 bit length
		if _receive_table.can_data_format_button.format_on():
			return "0x" + ("%02x" % byte).to_upper()
		else:
			return "0d" + ("%03d" % byte)


	func _formatted_frequency() -> String:
		if _receive_table.time_format_button.format_on():
			return "%.2f" % _frequency_hz
		else:
			return "%.2f" % (1000.0 / _frequency_hz)
