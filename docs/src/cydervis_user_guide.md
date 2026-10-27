# CyderVis User Guide

## Setup
- [Getting Started on Windows](getting_started_windows.md)
- [Getting Started on Linux](getting_started_linux.md)

## Connect and Receive Data in CyderVis

1. Open the **Interface** tab to see running CAN interfaces, their configured bitrate and estimated bus loading.
2. If your device is not running, choose **Add interface…**, select its adapter and bitrate, then click **Start** in the popup.
3. Click **Connect** on its row to begin receiving frames. Only one interface can be connected at a time; use **Disconnect** before selecting another.

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
  Ensure the interface is up in the running-interface list. Check the adapter selection and its canserver log if startup fails.
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

The Interface tab lists running interfaces with their bitrate, estimated bus
loading, **Connect / Disconnect** and **Stop** controls. Disconnecting leaves the
interface running. Stopping the connected interface disconnects it first. Other
interfaces continue running. Bus loading is observed independently of the active
connection, uses an estimate of classic CAN frame sizes and bit stuffing, and
shows **—** while the first measurement is pending or the bitrate is unknown.

Choose **Add interface…** to open the bring-up popup. Its single **Start** button
is disabled when the selected interface is already up; hover it to see why. On
Linux, select a SocketCAN device or **Create virtual CAN** for the next available
`vcanN` name. Interface changes use `ip` via `pkexec`; install iproute2 and polkit
with a desktop authentication agent.

On Windows, use current win-can-utils with protocol v1, `--list-adapters`, `--device`
and `--name` support. Select **gsusb**, **slcan** or **pcan**, choose a device from
the dropdown, then click **Start**. SLCAN lists the available COM ports;
gsusb lists connected USB adapter interfaces, including unavailable adapters.
PCAN lists attached USB, PCI and LAN channels through PCAN-Basic; install the PEAK
driver and matching-architecture `PCANBasic.dll`. Use **Refresh** after plugging in
or removing an adapter. Multiple devices require an explicit choice. Busy devices
can be inspected but cannot be brought up until their owning server is stopped.

For gsusb and PCAN, enter an **Adapter nickname** and click **Save name**. Clear
and save to remove it. Names and selections are stored in
`user://adapter_preferences.cfg` and survive application restarts and reboots.
gsusb names follow the serial number and USB interface, even if the USB address
changes. PCAN names follow the hardware GUID and controller where supported, or
a unique configured PCAN device ID and controller. On older PCAN hardware without
a GUID, assign a unique device ID using PEAK tools first. Missing, default or
duplicate identities cannot be nicknamed safely; the UI explains how to resolve
this. A missing saved adapter is never silently replaced by a different device.
The running-interface list uses the same label as the adapter dropdown, including
its nickname and device details. New service names use the nickname where present,
otherwise the device name (or COM port for SLCAN). Spaces and other characters are
converted to underscores for crosscan's pipe names; colliding names get a suffix.

CyderVis checks `CYDERVIS_CANSERVER` for an explicit executable path, then an
adjacent `win-can-utils/target/debug/canserver.exe` when running from a source
checkout, then PATH. For example, in PowerShell:

```powershell
$env:CYDERVIS_CANSERVER = 'D:\git\win-can-utils\target\debug\canserver.exe'
just run
```

CyderVis can start several servers and stops only the selected server from its
row. Servers it starts are also stopped when CyderVis exits. For externally
started servers, **Stop** brings the CAN interface down through the service API
and leaves the external process running; its row shows **Down**. Server diagnostics
are saved to `canserver_NAME.log` in the application's user-data directory. Use
**Connect** on a running row to connect through crosscan's versioned service
client. Linux continues to use SocketCAN directly.

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

MF4 capture uses a Rust writer compiled into CyderVis; no Python installation or
additional recording software is required. It writes uncompressed MDF 4.10 files
in bounded chunks directly to the recording folder, with periodic flushes during
capture. Stop recording or close CyderVis normally to save the final chunk.

## Viewing a saved MF4 capture

Select **Open MF4…** on the Trace tab to open a saved capture in its own viewer.
Choose a channel group to browse raw CAN frames or recorded signal values with
their units. **Previous**, **Next**, and the page number let you navigate the
entire group. Select a row and use **Copy row** to copy its values. Times are
relative to the file's start time, shown above the table in UTC.

For raw CAN files, **Load DBC…** adds decoded message and signal values to the
table. **Clear DBC** removes that decoding. Decoded MF4 signal files already
contain their values and do not need a DBC. The viewer can remain open while
using a live CAN interface; it reads files without transmitting their frames.
CAN FD payloads longer than eight bytes are displayed raw without DBC decoding.

The built-in Rust reader opens CyderVis recordings and row-based MDF 4 captures
with uncompressed, deflate, or transposed-deflate data, including files made by
the earlier Python recorder. Reads run in the background and only the requested
page is displayed. Unsupported layouts or conversions are reported explicitly.
The viewer supports up to 256 leaf channels per group, 65,536-byte records,
128 MiB decompressed blocks and 16 million indexed records in unsorted files.
Sorted recordings use direct access without a per-record index.

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
