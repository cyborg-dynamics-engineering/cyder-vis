extends LineEdit
class_name TransmitTableLineEdit

var _transmit_table: TransmitTable
var _row: Control
var _send_text_cell: Control


func _ready() -> void:
	_send_text_cell = get_parent()
	var ancestor: Node = get_parent()
	while ancestor != null and not ancestor is TransmitTable:
		ancestor = ancestor.get_parent()
	_transmit_table = ancestor
	_row = _send_text_cell
	while _row.get_parent() != _transmit_table.rows:
		_row = _row.get_parent()
	var column: Control = _send_text_cell
	while column.get_parent() != _row:
		column = column.get_parent()

	# Add movement up and down the transmit table rows using the Enter and Enter + Shift keys
	text_submitted.connect(
		func(_new_text: String):
			# Find the next row that should be focused
			var next_index: int
			if Input.is_key_pressed(KEY_SHIFT):
				# Move focus up a row, but if at the top already move focus to the bottom
				next_index = _row.get_index() - 1
				if next_index == 0:
					next_index = _transmit_table.rows.get_child_count() - 1
			else:
				# Move focus down a row, but if at the bottom already move focus to the top
				next_index = _row.get_index() + 1
				if next_index == _transmit_table.rows.get_child_count():
					next_index = 1

			# Change focus to this row
			var next_row = _transmit_table.rows.get_child(next_index)
			if next_row:
				for field: LineEdit in next_row.get_child(column.get_index()).find_children("*", "LineEdit", true, false):
					if field.editable and field.is_visible_in_tree():
						field.grab_focus()
						break
	)

	# Select all text when entering editing mode
	editing_toggled.connect(
		func(toggled_on: bool):
			if toggled_on:
				select_all()
	)

	# Deselect all text when losing focus
	focus_exited.connect(deselect)
