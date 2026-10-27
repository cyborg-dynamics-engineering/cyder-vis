extends Button
class_name SendButton


@export_category("Node References")
@export var transmit_table: TransmitTable
@export var use_dbc: bool = false


func _ready() -> void:
	self.pressed.connect(_button_pressed)


func _button_pressed() -> void:
	# Add a new send entry to the transmission table (Does not actually send any CAN data yet)
	if use_dbc:
		transmit_table._choose_dbc_message()
	else:
		transmit_table.add_new_send_entry()
