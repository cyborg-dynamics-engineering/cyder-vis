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
        "id": "00102",
        "extended_id": True,
        "signals" :{
            "id": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "SetCanBitrate",
        "id": "00202",
        "extended_id": True,
        "signals" :{
            "can_bitrate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "GetProductInfo",
        "id": "FFF02",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "ProductInfo",
        "id": "00002",
        "extended_id": True,
        "signals" :{
            "part_no": {"type": "u32", "unit": ""},
            "patch": {"type": "u16", "unit": ""},
            "minor": {"type": "u8", "unit": ""},
            "major": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "Status",
        "id": "90002",
        "extended_id": True,
        "signals" :{
            "status": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "ClearStatus",
        "id": "90102",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "WriteToFlash",
        "id": "00302",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "Ack",
        "id": "91002",
        "extended_id": True,
        "signals" :{
            "ident": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "Nack",
        "id": "90302",
        "extended_id": True,
        "signals" :{
            "status": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SoftReset",
        "id": "99802",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "Reboot",
        "id": "99902",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "SetStatusRate",
        "id": "90202",
        "extended_id": True,
        "signals" :{
            "status_rate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "ScalingFactors",
        "id": "10102",
        "extended_id": True,
        "signals" :{
            # TODO: Signals a f32 dbc should automatically add the SIG_VALTYPE line
            "accel_scale": {"type": "u32", "unit": ""},
            "gyro_scale": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "DoGyroCalibration",
        "id": "10202",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "SetComplementaryFilter",
        "id": "10302",
        "extended_id": True,
        "signals" :{
            "alpha": {"type": "u32", "unit": ""},
            "odr": {"type": "u8", "unit": ""},
            "output_type": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetAccelFilter",
        "id": "10402",
        "extended_id": True,
        "signals" :{
            "odr_div": {"type": "u8", "unit": ""},
            "lp_div": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetGyroFilter",
        "id": "10502",
        "extended_id": True,
        "signals" :{
            "odr_div": {"type": "u8", "unit": ""},
            "lp_div": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetAccelResolution",
        "id": "10602",
        "extended_id": True,
        "signals" :{
            "full_scale": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetGyroResolution",
        "id": "10702",
        "extended_id": True,
        "signals" :{
            "full_scale": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetDeviceOrientation",
        "id": "10802",
        "extended_id": True,
        "signals" :{
            "pitch": {"type": "u32", "unit": "degrees"},
            "roll": {"type": "u32", "unit": "degrees"}
        }
    },
    {
        "name": "DeviceId",
        "id": "40102",
        "extended_id": True,
        "signals" :{
            "id": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "GyroFilter",
        "id": "40202",
        "extended_id": True,
        "signals" :{
            "odr_div": {"type": "u8", "unit": ""},
            "lp_div": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "AccelFilter",
        "id": "40302",
        "extended_id": True,
        "signals" :{
            "odr_div": {"type": "u8", "unit": ""},
            "lp_div": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "AccelResolution",
        "id": "40402",
        "extended_id": True,
        "signals" :{
            "full_scale": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "GyroResolution",
        "id": "40502",
        "extended_id": True,
        "signals" :{
            "full_scale": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "ComplementaryFilter",
        "id": "40602",
        "extended_id": True,
        "signals" :{
            "alpha": {"type": "u32", "unit": ""},
            "odr": {"type": "u8", "unit": ""},
            "output_type": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "GetScalingFactors",
        "id": "40702",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "GetConfig",
        "id": "40802",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "GetSensorFreqFine",
        "id": "40902",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "SensorFreqFine",
        "id": "41002",
        "extended_id": True,
        "signals" :{
            "fine": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "GyroBias",
        "id": "41102",
        "extended_id": True,
        "signals" :{
            "x": {"type": "i16", "unit": ""},
            "y": {"type": "i16", "unit": ""},
            "z": {"type": "i16", "unit": ""},
        }
    },
    {
        "name": "CanSpeed",
        "id": "41202",
        "extended_id": True,
        "signals" :{
            "can_bitrate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "DeviceOrientation",
        "id": "41302",
        "extended_id": True,
        "signals" :{
            "pitch": {"type": "u32", "unit": "degrees"},
            "roll": {"type": "u32", "unit": "degrees"}
        }
    },
    {
        "name": "StatusRate",
        "id": "41402",
        "extended_id": True,
        "signals" :{
            "status_rate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "GyroData",
        "id": "80102",
        "extended_id": True,
        "signals" :{
            "x": {"type": "i16", "unit": ""},
            "y": {"type": "i16", "unit": ""},
            "z": {"type": "i16", "unit": ""},
        }
    },
    {
        "name": "AccelData",
        "id": "80202",
        "extended_id": True,
        "signals" :{
            "x": {"type": "i16", "unit": ""},
            "y": {"type": "i16", "unit": ""},
            "z": {"type": "i16", "unit": ""},
        }
    },
    {
        "name": "TiltAngles",
        "id": "80302",
        "extended_id": True,
        "signals" :{
            "pitch": {"type": "u32", "unit": "degrees"},
            "roll": {"type": "u32", "unit": "degrees"}
        }
    },
    {
        "name": "QuatWX",
        "id": "80402",
        "extended_id": True,
        "signals" :{
            "w": {"type": "u32", "unit": ""},
            "x": {"type": "u32", "unit": ""}
        }
    },
    {
        "name": "QuatYZ",
        "id": "80502",
        "extended_id": True,
        "signals" :{
            "y": {"type": "u32", "unit": ""},
            "z": {"type": "u32", "unit": ""}
        }
    },
    ]

    print(gen_dbc(messages))
