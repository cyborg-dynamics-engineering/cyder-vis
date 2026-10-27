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
