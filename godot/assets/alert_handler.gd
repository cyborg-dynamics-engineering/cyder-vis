extends Node
class_name AlertHandler

static var _handler_instance: AlertHandler

var prev_alert_box: AcceptDialog


func _ready() -> void:
	_handler_instance = self
	prev_alert_box = null


static func singleton() -> AlertHandler:
	return _handler_instance


static func display_error(msg: String) -> void:
	var handler_ref := singleton()
	if handler_ref == null:
		return
	
	# Only create new alert box if no old alerts are present
	if is_instance_valid(handler_ref.prev_alert_box):
		return

	# Load the monospaced font
	var mono_font: Font = load("res://fonts/JetBrainsMono-Regular.ttf")
	if mono_font == null:
		push_error("Failed to load JetBrainsMono-Regular.ttf")
		return

	var alert_box: AcceptDialog = AcceptDialog.new()
	alert_box.title = ""
	alert_box.dialog_text = msg
	alert_box.exclusive = false

	# Add to scene BEFORE querying children / get_label()
	singleton().get_tree().current_scene.add_child(alert_box)

	# Apply font directly to the internal Label and OK button
	var text_label: Label = alert_box.get_label()
	text_label.add_theme_font_override("font", mono_font)

	#var ok_button: Button = alert_box.get_ok_button()
	#ok_button.add_theme_font_override("font", mono_font)

	text_label.add_theme_font_size_override("font_size", 14)
	#ok_button.add_theme_font_size_override("font_size", 14)

	# Show + position
	alert_box.popup()  # or popup_centered()
	alert_box.position = Vector2i(
		(_handler_instance.get_viewport().size.x - alert_box.size.x) / 2,
		(_handler_instance.get_viewport().size.y - alert_box.size.y) - 50
	)

	# Have the alert box delete itself when closed
	alert_box.confirmed.connect(alert_box.queue_free)
	alert_box.close_requested.connect(alert_box.queue_free)

	handler_ref.prev_alert_box = alert_box
