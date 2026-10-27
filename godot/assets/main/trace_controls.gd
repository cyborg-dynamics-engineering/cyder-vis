class_name TraceControls
extends VBoxContainer

@export var can_bridge: GodotCanBridge

var _epochs: Array[CheckBox] = []
var _hardware: Label


func _ready() -> void:
	var timing := HFlowContainer.new()
	add_child(timing)
	var label := Label.new()
	label.text = "Time origin:"
	timing.add_child(label)
	var group := ButtonGroup.new()
	var labels := ["System epoch", "App time", "Restart on recording"]
	var tips := [
		"Seconds since 1970-01-01 UTC, aligned to the system clock at app launch.",
		"Seconds since the application started.",
		"Restart at zero when each recording starts. Before the first recording, use app time. Older or queued frames can have negative times."
	]
	for mode in labels.size():
		var radio := CheckBox.new()
		radio.text = labels[mode]
		radio.tooltip_text = tips[mode]
		radio.button_group = group
		radio.pressed.connect(_select_epoch.bind(mode))
		timing.add_child(radio)
		_epochs.append(radio)
	_hardware = Label.new()
	_hardware.visible = false
	timing.add_child(_hardware)

	var filters := HBoxContainer.new()
	add_child(filters)
	var input := LineEdit.new()
	input.placeholder_text = "Trace filter: 123:7FF,400:700"
	input.tooltip_text = "candump syntax: ID:MASK, ID~MASK, #ERROR_MASK, j (AND). Empty shows all data frames."
	input.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	filters.add_child(input)
	var apply := Button.new()
	apply.text = "Apply filter"
	filters.add_child(apply)
	var status := Label.new()
	filters.add_child(status)
	var update := func():
		var error: String = can_bridge.set_trace_filter(input.text)
		status.text = "Applied" if error.is_empty() else "Invalid filter"
		status.tooltip_text = error
		if not error.is_empty():
			AlertHandler.display_error(error)
	apply.pressed.connect(update)
	input.text_submitted.connect(func(_text: String): update.call())
	_process(0.0)


func _select_epoch(mode: int) -> void:
	var error := can_bridge.set_timestamp_epoch(mode)
	if not error.is_empty():
		AlertHandler.display_error(error)
	_process(0.0)


func _process(_delta: float) -> void:
	var recording := can_bridge.is_recording()
	var mode := can_bridge.get_timestamp_epoch()
	for index in _epochs.size():
		_epochs[index].disabled = recording
		_epochs[index].set_pressed_no_signal(index == mode)
	_update_hardware_indicator(can_bridge.get_hardware_timestamp_status())


func _update_hardware_indicator(status: int) -> void:
	_hardware.visible = status != 0
	match status:
		1:
			_hardware.text = "Hardware timestamps disabled"
			_hardware.tooltip_text = "This adapter supports hardware timestamps, but they are disabled in the CAN service."
		2:
			_hardware.text = "Hardware timestamps: waiting"
			_hardware.tooltip_text = "The adapter firmware supports hardware timestamps and they are enabled. Waiting for timestamped receive frames."
		3:
			_hardware.text = "Hardware timestamping enabled"
			_hardware.tooltip_text = "Confirmed adapter capability and hardware ticks received. Hardware intervals are preserved for every time origin."
		4:
			_hardware.text = "Hardware timestamps: host fallback"
			_hardware.tooltip_text = "The adapter supports hardware timestamps, but the latest receive frame had none. That frame uses host time."
	_hardware.modulate = Color(0.6, 0.85, 0.65) if status == 3 else Color(0.8, 0.8, 0.7)
