# Validation

Build the extension before running the Godot integration tests. The existing
godot-rust 0.4 binding supports Rust 1.88, which is also used in CI:

```text
cargo +1.88 test --manifest-path gdextension_can_io/Cargo.toml --locked --lib
cargo +1.88 build --manifest-path gdextension_can_io/Cargo.toml --locked
python godot/tests/run_tests.py /path/to/godot
```

MF4 writing is compiled into the extension and all native writer tests run
without Python. For independent read-back validation, install the optional test
tools and set `CYDERVIS_MF4_VERIFY_PYTHON` to that Python interpreter before
running Cargo tests:

```text
python -m pip install "python-can[mf4]>=4.6,<5" "asammdf>=8,<9"
```

The verifier reopens 6,013 recorded frames with python-can and asammdf, checking
raw data, flags, multiple interfaces, filtering, timestamps, units and decoded
channels across chunk boundaries. Only this independent verification step is
skipped without the environment variable. Python is never needed for recording.

The same interpreter generates independent reader fixtures with uncompressed,
deflate and transposed-deflate data. Native reader tests check paging, DBC
decoding, big-endian signed values, linear conversions, units, invalid samples
and exact 64-bit integers. Malformed links, truncated blocks and cancellation
are also tested without Python. Godot viewer tests use the compressed fixtures
in `godot/tests/mf4`; regenerate their contents with
`python gdextension_can_io/tests/make_mf4_fixtures.py /temporary/output` (the
viewer fixtures are `raw-2.mf4`, `signals-2.mf4` and `empty.mf4`, renamed to
`raw.mf4`, `signals.mf4` and `empty.mf4`). Opening files needs no Python runtime.

The Godot runner checks script errors and completion markers as well as exit
codes. Interface command planning is tested without changing the host's network
configuration; physical adapter operations still need Windows/Linux hardware
validation.

External-interface tests check that Add interface greys running/busy adapters
without saved names, including gsusb serial and USB-location aliases, normalized
COM/PCAN selectors, `auto` selection, multiple USB channels and up/down changes.
Opening the popup or changing service state refreshes availability; bus-loading
updates alone do not trigger another hardware probe.

Timestamp tests inject hardware ticks with deliberately different host/delivery
times and verify unchanged hardware spacing, zero/missing timestamps, reordered
samples, extended counter rollover and clock resets. A mock service exercises
the timestamp fields over the actual named-pipe protocol. Trace controls offer
system UNIX epoch, app time (default), and restart on recording. The selected
origin applies to trace, CSV and MF4, while hardware intervals remain unchanged.
Origin controls lock during recording. A failed start does not reset time; older
or queued samples may appear negative after restarting a recording origin.
MF4 headers identify the selected origin, keeping absolute UTC time consistent.
Native and optional independent readers check all three origins, raw/decoded
agreement, and recordings started an hour after app launch.

The Windows hardware indicator requires a gsusb/PCAN adapter with an explicitly
reported hardware timestamp capability. gsusb uses the firmware HW_TIMESTAMP
feature bit, not the model name. Unknown/unsupported adapters, SLCAN and virtual
interfaces have no indicator. Supported adapters show disabled, waiting for
hardware ticks, enabled, or host fallback according to service configuration and
received frames. Disconnect clears the indicator. The mock service checks these
states and all time origins through Godot on the actual named-pipe protocol.
The current Linux crosscan receive API does not expose hardware timestamp
metadata, so it does not advertise hardware timestamping in this UI.
