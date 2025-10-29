# This script is used to generate the body of a dbc file. 
# The intention is to define a list of dictionaries that describe the messages 
# and signals and write those to a file. See the example below.

def type_to_signed_size(type_str):
    signed = "-"
    if type_str[0] == "u":
        signed = "+"
    size = int(type_str[1:])
    return signed, size
    
def dict_to_dbc(message_dict):
    # Extract main fields
    name = message_dict["name"]
    can_id = int(message_dict["id"],16)

    # Compose CAN ID: msg_id (24 bits) shifted left by 8 + node_id (8 bits)
    can_id = int("80000000", 16) | can_id if message_dict["extended_id"] else can_id

    # Extract signals (all dict items that are dicts themselves)
    signals = message_dict["signals"]

    # Print the message line
    size = 0 # in bytes
    for k,v in signals.items():
        s, ss = type_to_signed_size(v["type"])
        size += ss/8

    dbc_lines = []
    dbc_lines.append(
        f'BO_ {can_id} {name}: {int(size)} Vector__XXX'
    )

    # Now print the signals
    bit_start = 0
    for sig_name,v in signals.items():
        # Only support u32 for now
        signed, size = type_to_signed_size(v["type"])
        unit = v["unit"]
        dbc_lines.append(
            f'    SG_ {sig_name} : {bit_start}|{size}@1{signed} (1,0) [0|0] \"{unit}\" Vector__XXX'
        )
        bit_start += size

    dbc_lines.append("\n")

    return "\n".join(dbc_lines)

def gen_dbc(messages):
    dbc = ""
    for m in messages:
        dbc += dict_to_dbc(m)
    return dbc

    
if __name__ == "__main__":

    # Example usage
    messages = [
    {
        "name": "SetDeviceId",
        "id": "20102",
        "extended_id": True,
        "signals" :{
            "id": {"type": "u32", "unit": ""},
        }
    },
    ]

    print(gen_dbc(messages))
