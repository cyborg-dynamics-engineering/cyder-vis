from gen_dbc import gen_dbc

messages = [
    {
        "name": "Reboot",
        "id": "10110",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "SoftReset",
        "id": "10210",
        "extended_id": True,
        "signals" :{
        }
    },
        {
        "name": "WriteToFlash",
        "id": "10310",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "ClearStatus",
        "id": "10410",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "SetDeviceId",
        "id": "20110",
        "extended_id": True,
        "signals" :{
            "id": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetCanBitrate",
        "id": "20210",
        "extended_id": True,
        "signals" :{
            "can_bitrate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetStatusRate",
        "id": "20310",
        "extended_id": True,
        "signals" :{
            "status_rate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "GetConfig",
        "id": "30210",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "Ack",
        "id": "40110",
        "extended_id": True,
        "signals" :{
            "ident": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "StatusRate",
        "id": "40310",
        "extended_id": True,
        "signals" :{
            "status_rate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "Status",
        "id": "70010",
        "extended_id": True,
        "signals" :{
            "status": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetOdr",
        "id": "A0110",
        "extended_id": True,
        "signals" :{
            "odr": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetSteinhartA",
        "id": "A0210",
        "extended_id": True,
        "signals" :{
            "a": {"type": "u32", "unit": ""},
        }
    },
        {
        "name": "SetSteinhartB",
        "id": "A0310",
        "extended_id": True,
        "signals" :{
            "b": {"type": "u32", "unit": ""},
        }
    },
        {
        "name": "SetSteinhartC",
        "id": "A0410",
        "extended_id": True,
        "signals" :{
            "c": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "Odr",
        "id": "C0110",
        "extended_id": True,
        "signals" :{
            "status_rate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SteinhartA",
        "id": "C0210",
        "extended_id": True,
        "signals" :{
            "a": {"type": "u32", "unit": ""},
        }
    },
        {
        "name": "SteinhartB",
        "id": "C0310",
        "extended_id": True,
        "signals" :{
            "b": {"type": "u32", "unit": ""},
        }
    },
        {
        "name": "SteinhartC",
        "id": "C0410",
        "extended_id": True,
        "signals" :{
            "c": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "Temperatures",
        "id": "E0010",
        "extended_id": True,
        "signals" :{
            "sensor_1": {"type": "i16", "unit": ""},
            "sensor_2": {"type": "i16", "unit": ""},
            "sensor_3": {"type": "i16", "unit": ""},
            "sensor_4": {"type": "i16", "unit": ""},
        }
    },
]

print(gen_dbc(messages))