"""Independent read-back of the Rust recorder's MF4 output."""
import sys

import can
import numpy as np
from asammdf import MDF

raw_path, decoded_path = sys.argv[1:]
frames = list(can.MF4Reader(raw_path))
assert len(frames) == 2053, len(frames)
configuration = [frame for frame in frames if frame.arbitration_id == 0x10302]
assert len(configuration) == 2050
assert configuration[0].is_extended_id
assert configuration[0].data == bytes.fromhex("6f12833a0b010000")
assert any(frame.is_remote_frame and frame.dlc == 8 for frame in frames)
assert any(frame.is_error_frame for frame in frames)
assert any(not frame.is_extended_id and frame.arbitration_id == 0x123 for frame in frames)
np.testing.assert_allclose(np.diff([frame.timestamp for frame in configuration]), 0.001, atol=1e-6)
with MDF(raw_path) as raw:
    epoch = raw.header.start_time.timestamp()
    assert abs(configuration[0].timestamp - epoch - 0.01) < 1e-6
with MDF(decoded_path) as decoded:
    alpha = decoded.get("SetComplementaryFilter.alpha")
    assert len(alpha.samples) == 2050
    np.testing.assert_allclose(alpha.samples, float(np.float32(0.001)), rtol=0, atol=0)
    np.testing.assert_allclose(alpha.timestamps, np.arange(2050) * 0.001 + 0.01, atol=1e-12)
    np.testing.assert_array_equal(decoded.get("SetComplementaryFilter.odr").samples, np.full(2050, 11))
print("MF4 read-back passed: raw, extended, remote, error, timestamps and decoded channels")
