"""Independent validation of MF4 origins, including UNIX epoch channel values."""
import sys

import can
import numpy as np
from asammdf import MDF

absolute = 1_710_000_000 + np.array([3599.999, 3600.005, 3600.006])
for index, path in enumerate(sys.argv[1:]):
    mode = index // 2
    epoch = [0, 1_710_000_000, 1_710_003_600][mode]
    expected = absolute - epoch
    with MDF(path) as capture:
        assert abs(capture.header.start_time.timestamp() - epoch) < 1e-6
        name = "SetComplementaryFilter.alpha" if index % 2 else "CAN_DataFrame"
        signal = capture.get(name)
        np.testing.assert_allclose(signal.timestamps, expected, rtol=0, atol=1e-6)
    if index % 2 == 0:
        frames = list(can.MF4Reader(path))
        np.testing.assert_allclose([f.timestamp for f in frames], absolute, rtol=0, atol=1e-6)
print("MF4 epoch read-back passed: system, app and recording origins, raw and decoded")
