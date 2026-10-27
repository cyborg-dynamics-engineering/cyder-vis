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

