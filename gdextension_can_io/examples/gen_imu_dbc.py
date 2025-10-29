from gen_dbc import gen_dbc

if __name__ == "__main__":

    # Example usage
    messages = [
    {
        "name": "Reboot",
        "id": "10102",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "SoftReset",
        "id": "10202",
        "extended_id": True,
        "signals" :{
        }
    },
        {
        "name": "WriteToFlash",
        "id": "10302",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "ClearStatus",
        "id": "10402",
        "extended_id": True,
        "signals" :{
        }
    },

    {
        "name": "SetDeviceId",
        "id": "20102",
        "extended_id": True,
        "signals" :{
            "id": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "SetCanBitrate",
        "id": "20202",
        "extended_id": True,
        "signals" :{
            "can_bitrate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetStatusRate",
        "id": "20302",
        "extended_id": True,
        "signals" :{
            "status_rate": {"type": "u8", "unit": ""},
        }
    },
        {
        "name": "GetProductInfo",
        "id": "30102",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "GetConfig",
        "id": "30202",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "Ack",
        "id": "40102",
        "extended_id": True,
        "signals" :{
            "ident": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "ProductInfo",
        "id": "40202",
        "extended_id": True,
        "signals" :{
            "part_no": {"type": "u32", "unit": ""},
            "patch": {"type": "u16", "unit": ""},
            "minor": {"type": "u8", "unit": ""},
            "major": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "StatusRate",
        "id": "40302",
        "extended_id": True,
        "signals" :{
            "status_rate": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "Status",
        "id": "70002",
        "extended_id": True,
        "signals" :{
            "status": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "DoGyroCalibration",
        "id": "90102",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "SetAccelFilter",
        "id": "A0102",
        "extended_id": True,
        "signals" :{
            "odr_div": {"type": "u8", "unit": ""},
            "lp_div": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetGyroFilter",
        "id": "A0202",
        "extended_id": True,
        "signals" :{
            "odr_div": {"type": "u8", "unit": ""},
            "lp_div": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetAccelResolution",
        "id": "A0302",
        "extended_id": True,
        "signals" :{
            "full_scale": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetGyroResolution",
        "id": "A0402",
        "extended_id": True,
        "signals" :{
            "full_scale": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetComplementaryFilter",
        "id": "A0502",
        "extended_id": True,
        "signals" :{
            "alpha": {"type": "u32", "unit": ""},
            "odr": {"type": "u8", "unit": ""},
            "output_type": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "SetDeviceOrientation",
        "id": "A0602",
        "extended_id": True,
        "signals" :{
            "pitch": {"type": "u32", "unit": "degrees"},
            "roll": {"type": "u32", "unit": "degrees"}
        }
    },
    {
        "name": "GetScalingFactors",
        "id": "B0102",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "GetSensorFreqFine",
        "id": "B0202",
        "extended_id": True,
        "signals" :{
        }
    },
    {
        "name": "AccelFilter",
        "id": "C0102",
        "extended_id": True,
        "signals" :{
            "odr_div": {"type": "u8", "unit": ""},
            "lp_div": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "GyroFilter",
        "id": "C0202",
        "extended_id": True,
        "signals" :{
            "odr_div": {"type": "u8", "unit": ""},
            "lp_div": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "AccelResolution",
        "id": "C0302",
        "extended_id": True,
        "signals" :{
            "full_scale": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "GyroResolution",
        "id": "C0402",
        "extended_id": True,
        "signals" :{
            "full_scale": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "ComplementaryFilter",
        "id": "C0502",
        "extended_id": True,
        "signals" :{
            "alpha": {"type": "u32", "unit": ""},
            "odr": {"type": "u8", "unit": ""},
            "output_type": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "DeviceOrientation",
        "id": "C0602",
        "extended_id": True,
        "signals" :{
            "pitch": {"type": "u32", "unit": "degrees"},
            "roll": {"type": "u32", "unit": "degrees"}
        }
    },
    {
        "name": "ScalingFactors",
        "id": "C1102",
        "extended_id": True,
        "signals" :{
            # TODO: Signals a f32 dbc should automatically add the SIG_VALTYPE line
            "accel_scale": {"type": "u32", "unit": ""},
            "gyro_scale": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "SensorFreqFine",
        "id": "C1202",
        "extended_id": True,
        "signals" :{
            "fine": {"type": "u32", "unit": ""},
        }
    },
    {
        "name": "GyroBias",
        "id": "C2102",
        "extended_id": True,
        "signals" :{
            "x": {"type": "i16", "unit": ""},
            "y": {"type": "i16", "unit": ""},
            "z": {"type": "i16", "unit": ""},
        }
    },
    {
        "name": "AccelData",
        "id": "E0102",
        "extended_id": True,
        "signals" :{
            "x": {"type": "i16", "unit": ""},
            "y": {"type": "i16", "unit": ""},
            "z": {"type": "i16", "unit": ""},
        }
    },
    {
        "name": "GyroData",
        "id": "E0202",
        "extended_id": True,
        "signals" :{
            "x": {"type": "i16", "unit": ""},
            "y": {"type": "i16", "unit": ""},
            "z": {"type": "i16", "unit": ""},
        }
    },
    {
        "name": "TiltAngles",
        "id": "E0302",
        "extended_id": True,
        "signals" :{
            "pitch": {"type": "u32", "unit": "degrees"},
            "roll": {"type": "u32", "unit": "degrees"}
        }
    },
    {
        "name": "QuatWX",
        "id": "E0402",
        "extended_id": True,
        "signals" :{
            "w": {"type": "u32", "unit": ""},
            "x": {"type": "u32", "unit": ""}
        }
    },
    {
        "name": "QuatYZ",
        "id": "E0502",
        "extended_id": True,
        "signals" :{
            "y": {"type": "u32", "unit": ""},
            "z": {"type": "u32", "unit": ""}
        }
    },
    {
        "name": "DeviceId",
        "id": "FFE02",
        "extended_id": True,
        "signals" :{
            "id": {"type": "u8", "unit": ""},
        }
    },
    {
        "name": "CanSpeed",
        "id": "FFF02",
        "extended_id": True,
        "signals" :{
            "can_bitrate": {"type": "u8", "unit": ""},
        }
    },
    ]

    print(gen_dbc(messages))
