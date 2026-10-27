extends HBoxContainer

@export var can_bridge: GodotCanBridge


func _ready() -> void:
	var input := LineEdit.new()
	input.placeholder_text = "Trace filter: 123:7FF,400:700"
	input.tooltip_text = "candump syntax: ID:MASK, ID~MASK, #ERROR_MASK, j (AND). Empty shows all data frames."
	input.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	add_child(input)
	var apply := Button.new()
	apply.text = "Apply filter"
	add_child(apply)
	var status := Label.new()
	add_child(status)
	var update := func():
		var error: String = can_bridge.set_trace_filter(input.text)
		status.text = "Applied" if error.is_empty() else "Invalid filter"
		status.tooltip_text = error
		if not error.is_empty():
			AlertHandler.display_error(error)
	apply.pressed.connect(update)
	input.text_submitted.connect(func(_text: String): update.call())
