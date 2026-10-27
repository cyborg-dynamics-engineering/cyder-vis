# Validation

Build the extension before running the Godot integration tests. The existing
godot-rust 0.3 binding needs Rust 1.88 with current Rust toolchains:

```text
cargo +1.88 test --manifest-path gdextension_can_io/Cargo.toml --locked --lib
cargo +1.88 build --manifest-path gdextension_can_io/Cargo.toml --locked
python godot/tests/run_tests.py /path/to/godot
```

To also run the MF4 write/read integration test, install the optional MF4
dependencies described in the user guide and set `CYDERVIS_MF4_PYTHON` to that
Python interpreter before running Cargo tests. This test reopens 2,053 recorded
frames with python-can and asammdf, checking raw data, flags, timestamps and
decoded channels across multiple batches. Without the environment variable, it
is skipped. CSV and filter tests do not need Python.

The Godot runner checks script errors and completion markers as well as exit
codes. Interface command planning is tested without changing the host's network
configuration; physical adapter operations still need Windows/Linux hardware
validation.
