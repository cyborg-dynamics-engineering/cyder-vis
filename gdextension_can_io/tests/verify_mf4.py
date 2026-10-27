"""Independent read-back of the Rust recorder's MF4 output."""
import sys

import can
import numpy as np
from asammdf import MDF

raw_path, decoded_path = sys.argv[1:]
frames = list(can.MF4Reader(raw_path))
assert len(frames) == 6013, len(frames)
configuration = [frame for frame in frames if frame.arbitration_id == 0x10302
                 and frame.data == bytes.fromhex("6f12833a0b010000")]
assert len(configuration) == 6000
assert all(frame.is_extended_id and frame.dlc == 8 for frame in configuration)
remote = [frame for frame in frames if frame.is_remote_frame]
assert [(frame.arbitration_id, frame.dlc, frame.is_extended_id) for frame in remote] == [
    (0x124, 8, False), (0x1ABCDE, 3, True)]
assert all(not frame.data for frame in remote)
errors = [frame for frame in frames if frame.is_error_frame]
assert len(errors) == 1 and errors[0].arbitration_id == 4
short = [frame for frame in frames if frame.arbitration_id == 0x123 and frame.dlc < 3]
assert [frame.data for frame in short] == [bytes([0x12]), bytes()]
assert all(not frame.is_extended_id for frame in short)
auxiliary = [frame for frame in frames if frame.arbitration_id == 0x123 and frame.dlc == 3]
assert len(auxiliary) == 6
assert all(frame.channel == 1 and frame.data == bytes([123, 0, 246]) for frame in auxiliary)
assert all(frame.channel == 0 for frame in configuration)
assert not any(frame.arbitration_id == 0x456 for frame in frames)
np.testing.assert_allclose(np.diff([frame.timestamp for frame in configuration]), 0.001, atol=1e-6)
with MDF(raw_path) as raw:
    start_time = raw.header.start_time
    epoch = start_time.timestamp()
    assert abs(configuration[0].timestamp - epoch - 0.01) < 1e-6
with MDF(decoded_path) as decoded:
    alpha = decoded.get("SetComplementaryFilter.alpha")
    assert start_time == decoded.header.start_time
    assert len(alpha.samples) == 6000
    np.testing.assert_allclose(alpha.samples, float(np.float32(0.001)), rtol=0, atol=0)
    np.testing.assert_allclose(alpha.timestamps, np.arange(6000) * 0.001 + 0.01, atol=1e-12)
    np.testing.assert_array_equal(decoded.get("SetComplementaryFilter.odr").samples, np.full(6000, 11))
    voltage = decoded.get("Auxiliary.voltage")
    temperature = decoded.get("Auxiliary.temperature")
    assert voltage.unit == "V" and temperature.unit == "°C"
    np.testing.assert_allclose(voltage.samples, np.full(6, 12.3), atol=1e-12)
    np.testing.assert_array_equal(temperature.samples, np.full(6, -45))
    np.testing.assert_allclose(voltage.timestamps, np.arange(6) + 0.0105, atol=1e-12)
print("MF4 read-back passed: 6,013 frames, chunk rollover, interfaces, flags, filters, timestamps and signals")
