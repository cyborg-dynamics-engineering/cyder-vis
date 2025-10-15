from gen_dbc import gen_dbc

messages = [
    {
        "name": "SetDeviceId",
        "id": "00110",
        "extended_id": True,
        "signals" :{
            "id": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetCanBitrate",
        "id": "00210",
        "extended_id": True,
        "signals" :{
            "can_bitrate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetStatusRate",
        "id": "0x90210",
        "extended_id": True,
        "signals" :{
            "status_rate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetOdr",
        "id": "99810",
        "extended_id": True,
        "signals" :{
            "odr": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SoftReset",
        "id": "99810",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "ClearStatus",
        "id": "90110",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "Reboot",
        "id": "99910",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "WriteToFlash",
        "id": "00310",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "Status",
        "id": "90010",
        "extended_id": True,
        "signals" :{
            "status": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "Temperatures",
        "id": "10010",
        "extended_id": True,
        "signals" :{
            "sensor_1": {"type": "u16", "unit": ""},
            "sensor_2": {"type": "u16", "unit": ""},
            "sensor_3": {"type": "u16", "unit": ""},
            "sensor_4": {"type": "u16", "unit": ""},
        }
    },

    {
        "name": "Ack",
        "id": "91010",
        "extended_id": True,
        "signals" :{
            "ident": {"type": "u32", "unit": ""},
        }
    },

]
print(gen_dbc(messages))