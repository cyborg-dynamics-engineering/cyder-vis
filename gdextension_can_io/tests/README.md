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

The Godot runner checks script errors and completion markers as well as exit
codes. Interface command planning is tested without changing the host's network
configuration; physical adapter operations still need Windows/Linux hardware
validation.
