extends Button
class_name ConversionCalculator


static func convert_f32(input: String, big_endian: bool = false) -> String:
	if not input.strip_edges().is_valid_float():
		return ""
	var value := input.to_float()
	if not is_finite(value) or abs(value) > 3.4028234663852886e38:
		return ""
	var buffer := StreamPeerBuffer.new()
	buffer.big_endian = big_endian
	buffer.put_float(value)
	var hex := buffer.data_array.hex_encode()
	var bytes := PackedStringArray()
	for i in range(0, hex.length(), 2):
		bytes.append(hex.substr(i, 2))
	return " ".join(bytes)


func _ready() -> void:
	pressed.connect(_open_calculator)


func _open_calculator() -> void:
	var dialog := AcceptDialog.new()
	dialog.title = "Float32 to CAN bytes"
	add_child(dialog)
	var form := VBoxContainer.new()
	form.custom_minimum_size = Vector2(400, 150)
	dialog.add_child(form)
	var input := LineEdit.new()
	input.placeholder_text = "Float32 value, e.g. 0.001"
	form.add_child(input)
	var byte_order := OptionButton.new()
	byte_order.add_item("Little-endian")
	byte_order.add_item("Big-endian")
	form.add_child(byte_order)
	var output := LineEdit.new()
	output.editable = false
	output.placeholder_text = "Enter a finite float32 value"
	form.add_child(output)
	var copy := Button.new()
	copy.text = "Copy hex bytes"
	copy.disabled = true
	form.add_child(copy)
	var update := func():
		output.text = convert_f32(input.text, byte_order.selected == 1)
		copy.disabled = output.text.is_empty()
	input.text_changed.connect(func(_value: String): update.call())
	byte_order.item_selected.connect(func(_index: int): update.call())
	copy.pressed.connect(func(): DisplayServer.clipboard_set(output.text))
	dialog.confirmed.connect(dialog.queue_free)
	dialog.canceled.connect(dialog.queue_free)
	dialog.popup_centered()
	input.grab_focus()
