# CyderVis User Guide

## Setup
- [Getting Started on Windows](getting_started_windows.md)
- [Getting Started on Linux](getting_started_linux.md)

## Connect and Receive Data in CyderVis

1. In **CyderVis**, locate the **CAN Interface** input field.  
2. Verify that the interface name matches the one brought up in the previous step (default is `can0`).  
3. Click **Start** to begin receiving data from the CAN adapter.

If other devices are active on the bus, incoming CAN frames will begin streaming into CyderVis and appear in the viewing table.

![CyderVis Streaming](images/cydervis_streaming.png)

---

## Adjust Display Options

Use the display option toggles to customize how CAN data is displayed.  
These toggles control message formatting and visualization preferences.

![CyderVis Display Options](images/cydervis_display_options.png)

---

## Load a `.dbc` File to Decode Frames

A **DBC file** defines how to decode CAN frames into human-readable signals.  
You can learn more about the DBC format here:  
<https://docs.openvehicles.com/en/latest/components/vehicle_dbc/docs/dbc-primer.html>

We provide example `.dbc` files for Cyder products at:  
<https://github.com/cyborg-dynamics-engineering/cyder-vis/tree/main/gdextension_can_io/examples>

To load a `.dbc` file:
1. Click **Open** next to the **DBC File** dialog.
2. Select your `.dbc` file.
3. Once loaded, decoded frames will automatically appear in the viewing table.

![CyderVis Decoded Frames](images/cydervis_decoded_frames.png)

---

## Plotting Decoded Data

1. Open the **Plot** tab.  
2. Click on any decoded variable name in the viewing table to toggle it on or off in the plot view.

![CyderVis Plotting](images/cydervis_plotting.png)

> 💡 **Note:**  
> Data must be decoded using a .dbc file to enable plotting.

---

## Step 7: Transmitting Frames

1. Open the **Transmit** tab.
2. Press 'Add New' to create a new message.
3. Fill out the Cycle Time, Frame ID and Data fields.
4. For extended IDs, select the EXT ID checkbox.
5. Click the 'Send' checkbox to begin transmitting.

> 💡 **Notes:**  
> 1. A **Cycle Time** of `0` designates a *one-shot* message — it sends once each time you click the **Send** checkbox.  
> 2. Messages cannot be edited whilst sending.

## Troubleshooting

- **CyderVis doesn’t connect:**  
  Ensure the `canserver` is running and the interface name matches (e.g., `can0`).
## Transmit messages from the DBC

Load the same DBC used for decoding on the Interface tab, then select **Add DBC
message** on the Transmit tab. Choose a message to populate its ID, frame format
and named signal fields. Messages start with sending disabled. Enter physical
values for floats and scaled signals; unscaled unsigned fields accept decimal
integers or `0x`-prefixed hex. Bounds, bit widths and scaling are checked before
transmission. **Add New** remains available for raw hex frames.

DBC float32/float64 signals must declare `SIG_VALTYPE_` (1 for float32, 2 for
float64). Extended IDs in DBC files include bit 31: ID `0x10302` is written as
`2147549954`. For example:

```text
BO_ 2147549954 SetComplementaryFilter: 8 PC
 SG_ alpha : 0|32@1+ (1,0) [0.001|0.1] "" Device
 SG_ odr : 32|8@1+ (1,0) [0|11] "" Device
 SG_ output_type : 40|8@1+ (1,0) [0|1] "" Device
SIG_VALTYPE_ 2147549954 alpha : 1;
```

Replacing or clearing the DBC disables existing DBC transmit rows; add them again
to use the new definitions. Multiplexed, overlapping or CAN FD definitions show a
reason they cannot be selected; use raw hex for unsupported classic CAN layouts.
Ranges with equal minimum and maximum are treated as unspecified.

## Interface controls

On the Interface tab, set the name and bitrate, then use **Bring up / Restart**.
Pause the connection before changing an interface. On Linux this uses `ip` via
`pkexec`; install iproute2 and polkit with a desktop authentication agent. For a
virtual bus, enter a name such as `vcan0`, then choose **Create vcan**. Existing
virtual buses can be restarted with **Virtual CAN** selected.

On Windows, install win-can-utils with `canserver.exe` on PATH, choose the driver
and optional adapter/channel, then use **Bring up / Restart**. This requires a
version supporting `--name`. CyderVis manages the server it starts and can restart
or stop it; an externally started server must be stopped in its own terminal.
Server diagnostics are saved to `canserver.log` in the application's user-data
directory. With multiple gsusb adapters, supply a specific adapter/channel.
Once the interface is ready, use **Start** to connect CyderVis.

## Recording CSV or MF4

On the Trace tab, choose a recording folder, **Raw frames**, **Decoded signals**
or **Raw + decoded**, and the **CSV / MF4 radio toggle**. Connect to the CAN bus,
then select **Start recording**. **Stop recording**, pausing the connection and
closing the application finalize the files. Wait for finalization before opening
an MF4 file. Both modes create separate raw and decoded files with unique names.
The controls show their paths; existing recordings are never overwritten.

Every received frame that matches the active trace filter is eligible for
recording, including traffic beyond the trace's 100-frame display history.
Applying a different trace filter changes subsequent recorded traffic. Decoded
capture requires the loaded DBC. Unknown, remote and error frames, and frames with
truncated or non-finite signals, do not produce decoded samples. Values are saved
before display rounding, with their units.

CSV raw columns contain timestamp in microseconds since app startup, interface,
hex CAN ID, frame flags, DLC and hex bytes. Decoded CSV has one row per signal
with message and signal names, value and unit. MF4 raw files use the standard CAN
bus logging layout; decoded files contain time series named `Message.Signal`.
MF4 time channels use seconds with a common file start time for both files.

MF4 capture uses Python 3 with `python-can` and `asammdf`. Install them in the
interpreter used by CyderVis:

```text
python -m pip install "python-can[mf4]>=4.6,<5" "asammdf>=8,<9"
```

Set `CYDERVIS_MF4_PYTHON` to a full Python executable path to use a virtual
environment. A missing interpreter or dependency is reported when starting MF4
capture. CSV recording has no Python dependency. MF4 diagnostics are saved next
to the recording in a `*-mf4.log` file. MF4 uses temporary storage during capture
and produces the final files on stop; close CyderVis normally to finalize them.

## Trace filters

Use **Apply filter** on the Trace tab to select traffic with candump syntax. An
empty field shows all data frames. Comma-separated rules use OR; add `j` to use
AND. Invalid input leaves the previous filter active. Filtering does not change
the receive table or plots.

Examples: `123:7FF` selects ID 123 (including extended and remote frames);
`123:C00007FF` selects only standard data frames with that ID;
`00000123:DFFFFFFF` selects only extended ID 123; `400~700` excludes IDs matching
that mask. `0~0,#FFFFFFFF` selects error frames only, when supplied by the CAN
backend. An optional interface prefix such as `can0,123:7FF` is accepted.

See the [candump manual](https://manpages.debian.org/testing/can-utils/candump.1.en.html)
for ID, mask, inversion, error-mask and join semantics.
