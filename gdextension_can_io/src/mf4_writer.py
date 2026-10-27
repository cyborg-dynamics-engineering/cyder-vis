"""Embedded MF4 worker. The parent sends filtered frame records as JSON lines."""
import json
import sys
from datetime import datetime, timezone


def capture(raw_path, decoded_path, stream):
    import can
    import numpy as np
    from asammdf import MDF, Signal

    raw = can.MF4Writer(raw_path, compression_level=1) if raw_path else None
    decoded = MDF(version="4.10") if decoded_path else None
    epoch = raw._mdf.header.start_time if raw else datetime.now(timezone.utc)
    if decoded:
        decoded.header.start_time = epoch
    groups = {}
    pending = {}

    def flush(key):
        records = pending.pop(key, [])
        if not records:
            return
        timestamps = np.array([record[0] for record in records], dtype=np.float64)
        columns = [np.array([record[1][i]["value"] for record in records], dtype=np.float64)
                   for i in range(len(records[0][1]))]
        if key not in groups:
            signals = [Signal(samples=column, timestamps=timestamps,
                              name=f"{key[2]}.{info['name']}", unit=info["unit"])
                       for column, info in zip(columns, records[0][1])]
            groups[key] = decoded.append(signals, acq_name=f"{key[0]:X} {'extended' if key[1] else 'standard'} {key[2]}", common_timebase=True)
        else:
            decoded.extend(groups[key], [(timestamps, None)] + [(column, None) for column in columns])

    try:
        print("READY", flush=True)
        for line in stream:
            record = json.loads(line)
            timestamp = record["timestamp_us"] * 1e-6
            if raw:
                raw(can.Message(timestamp=epoch.timestamp() + timestamp, arbitration_id=record["id"],
                                is_extended_id=record["extended"], is_remote_frame=record["remote"],
                                is_error_frame=record["error"], dlc=record["dlc"],
                                data=record["data"], channel=0))
            if decoded and record["signals"]:
                key = (record["id"], record["extended"], record["message"])
                pending.setdefault(key, []).append((timestamp, record["signals"]))
                if len(pending[key]) >= 1024:
                    flush(key)
    finally:
        try:
            if raw:
                raw.stop()
        finally:
            if decoded:
                try:
                    for key in list(pending):
                        flush(key)
                    decoded.save(decoded_path, overwrite=True)
                finally:
                    decoded.close()


if __name__ == "__main__":
    try:
        capture(sys.argv[1], sys.argv[2], sys.stdin)
    except Exception as error:
        print(f"MF4 recording failed: {error}", file=sys.stderr, flush=True)
        sys.exit(1)
