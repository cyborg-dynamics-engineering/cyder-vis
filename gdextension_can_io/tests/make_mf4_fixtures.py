"""Generate independent MF4 reader fixtures using python-can and asammdf.

Python is only used by tests; the application opens all these files in Rust.
"""
from pathlib import Path
from datetime import datetime, timezone
import sys

import can
import numpy as np
from asammdf import MDF, Signal
from asammdf.blocks.v4_blocks import ChannelConversion


def generate(destination):
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    for compression in range(3):
        writer = can.MF4Writer(destination / f"raw-{compression}.mf4", compression_level=compression)
        writer._start_time = writer.last_timestamp = 1_710_000_000
        writer._mdf.header.start_time = datetime.fromtimestamp(1_710_000_000, timezone.utc)
        writer._mdf.configure(write_fragment_size=2048)
        for index in range(2050):
            writer(can.Message(timestamp=1_710_000_000 + 0.01 + index * 0.001,
                               arbitration_id=0x10302, is_extended_id=True, channel=2,
                               data=bytes.fromhex("6f12833a0b010000")))
        for message in [
            can.Message(arbitration_id=0x123, is_extended_id=False, data=b""),
            can.Message(arbitration_id=0x1ABCDE, is_extended_id=True, is_fd=True,
                        bitrate_switch=True, error_state_indicator=True, data=bytes(range(12))),
            can.Message(arbitration_id=0x124, is_extended_id=False, is_remote_frame=True, dlc=8),
            can.Message(arbitration_id=4, is_extended_id=False, is_error_frame=True, data=b"\0" * 8),
        ]:
            message.timestamp = 1_710_000_003
            message.channel = 2
            writer(message)
        writer.stop()

        with MDF(version="4.10") as decoded:
            decoded.header.start_time = datetime.fromtimestamp(1_710_000_000, timezone.utc)
            timestamps = np.arange(2050, dtype=np.float64) * 0.001 + 0.01
            decoded.append([
                Signal(samples=np.arange(2050, dtype=np.uint16), timestamps=timestamps,
                       name="Battery.voltage", unit="V",
                       conversion=ChannelConversion(conversion_type=1, a=0.1, b=-40)),
                Signal(samples=np.full(2050, -1234, dtype=">i2"), timestamps=timestamps,
                       name="Motor.temperature", unit="°C"),
                Signal(samples=np.full(2050, 0.12345678912345678), timestamps=timestamps,
                       name="Motor.precise", invalidation_bits=(np.arange(2050) == 10)),
                Signal(samples=np.full(2050, 0xFFFF_FFFF_FFFF_FFFF, dtype=np.uint64), timestamps=timestamps,
                       name="Counter.u64"),
            ], common_timebase=True, acq_name="Independent signals")
            decoded.configure(write_fragment_size=2048)
            decoded.save(destination / f"signals-{compression}.mf4", compression=compression, overwrite=True)
    with MDF(version="4.10") as empty:
        empty.header.start_time = datetime.fromtimestamp(1_710_000_000, timezone.utc)
        empty.save(destination / "empty.mf4", overwrite=True)
    (destination / "broken.mf4").write_bytes(b"not an MDF file")


if __name__ == "__main__":
    generate(sys.argv[1])
