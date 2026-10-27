# CyderVis - A CAN Visualization tool for Windows & Linux

Cyder-Vis is an open-source visualizer and debugger for CAN bus networks. It helps engineers and developers receive, decode, transmit, and plot CAN messages for debugging and diagnostics.

<p align="center">
    <img src="godot/screenshot.png" width="50%">
</p>

## Documentation

###  User Guide

For installation and usage instructions, read the docs!

**Link:** https://cyborg-dynamics-engineering.github.io/cyder-vis/

### Video Tutorial

A video tutorial is available for how to use v0.0.4 of the software.

**Link:** https://youtu.be/ltGhDHuehn8?si=pxIEBTlz7TYvtH6-

## License

Cyder Vis is licensed under either of

    Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
    MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)


## Developers

### Project Structure

The visualiser tool is Godot based, with the Godot project located in the `godot/` directory. The `gdextension_can_io/` directory contains a rust-based GDExtension that provides a CAN interface node to Godot projects. The extension supports deserialisation of known CAN messages through the [CAN DBC](https://www.csselectronics.com/pages/can-dbc-file-database-intro) file format.

### Setup

- Ensure the all repo files are updated,

```bash
git lfs pull && git lfs checkout
```

- Install [Rust](https://www.rust-lang.org/tools/install) **(You may need to restart your computer on windows)**
- Build the CAN IO GDExtension with,

```bash
cd gdextension_can_io
cargo build
```

- Download the latest [Godot Editor](https://godotengine.org/)
- Open the project in the Godot editor. The project file can be found under `godot/project.godot`. Press `F5` or click the play button on the top right to run the project.

### Running with just

With Rust, Godot 4.5 or later, and [just](https://github.com/casey/just) installed, run from the repository root:

```bash
just run
```

This builds the CAN IO GDExtension, imports the Godot project resources, and launches CyderVis. Running `just` without a recipe does the same thing. Use `just editor` to build the extension and open the Godot editor, or `just build` to build only the extension.

The recipes use `godot` from your `PATH` by default. If your executable has a different name or location, override it with the `godot` variable:

```bash
just godot=godot4 run
```

On Windows, a full executable path can include spaces:

```powershell
just 'godot=C:\Program Files\Godot\Godot_v4.5-stable_win64.exe' run
```

You can also set the `GODOT_BIN` environment variable to the executable path. The existing Git LFS setup step is required before the first run.

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
