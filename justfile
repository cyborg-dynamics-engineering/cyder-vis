set windows-shell := ["cmd.exe", "/c"]

godot := env_var_or_default("GODOT_BIN", "godot")

default: run

# Build the CAN I/O GDExtension.
build:
    cargo build --manifest-path gdextension_can_io/Cargo.toml

# Build the extension, import resources, and launch CyderVis.
run: build
    "{{godot}}" --headless --path godot --import
    "{{godot}}" --path godot

# Build the extension and open the project in the Godot editor.
editor: build
    "{{godot}}" --editor --path godot
