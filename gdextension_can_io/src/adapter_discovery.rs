//! Read-only hardware discovery. Probes run off the Godot thread.
use serde::Serialize;
use std::path::PathBuf;
use std::thread::JoinHandle;

#[derive(Debug, Serialize)]
pub struct Adapter {
    pub device_name: String,
    pub selector: String,
    pub label: String,
    pub identity: String,
    pub identity_note: String,
    pub available: bool,
    pub reason: String,
}

#[derive(Default)]
pub struct AdapterDiscovery {
    job: Option<JoinHandle<String>>,
}

impl AdapterDiscovery {
    pub fn start(&mut self, driver: String) -> Result<(), String> {
        if self.job.is_some() {
            return Err("An adapter scan is already running.".into());
        }
        if !["gsusb", "slcan", "pcan", "socketcan"].contains(&driver.as_str()) {
            return Err("Select gsusb, slcan or pcan.".into());
        }
        self.job = Some(std::thread::spawn(move || {
            match discover(&driver) {
                Ok(adapters) => serde_json::json!({"driver": driver, "adapters": adapters}),
                Err(error) => serde_json::json!({"driver": driver, "error": error}),
            }
            .to_string()
        }));
        Ok(())
    }

    pub fn poll(&mut self) -> String {
        if self.job.as_ref().is_some_and(|job| job.is_finished()) {
            return self.job.take().unwrap().join().unwrap_or_else(|_| {
                serde_json::json!({"error": "Adapter scan failed unexpectedly."}).to_string()
            });
        }
        String::new()
    }
}

/// Explicit override, adjacent development checkout, then installed tools.
pub fn canserver_executable() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CYDERVIS_CANSERVER") {
        let path = PathBuf::from(path);
        return path
            .is_file()
            .then_some(path)
            .ok_or_else(|| "CYDERVIS_CANSERVER must point to an existing canserver.exe.".into());
    }
    if let Ok(directory) = std::env::current_dir() {
        // Support launching from either the repository root or its godot directory.
        for root in [directory.clone(), directory.join("..")] {
            let manifest = root.join("gdextension_can_io/Cargo.toml");
            let development = root.join("../win-can-utils/target/debug/canserver.exe");
            if manifest.is_file() && development.is_file() {
                return Ok(development);
            }
        }
    }
    crate::system_tools::find_in_path(
        "canserver.exe",
        &std::env::var_os("PATH").unwrap_or_default(),
    )
    .ok_or_else(|| {
        "Install current win-can-utils and put canserver.exe on PATH, or set CYDERVIS_CANSERVER."
            .into()
    })
}

fn discover(driver: &str) -> Result<Vec<Adapter>, String> {
    #[cfg(windows)]
    match driver {
        "gsusb" => {
            use std::os::windows::process::CommandExt;
            let output = std::process::Command::new(canserver_executable()?)
                .args(["gsusb", "--list-adapters"])
                .creation_flags(0x08000000)
                .output()
                .map_err(|e| format!("Cannot list gsusb adapters: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "Cannot list gsusb adapters. Update win-can-utils (requires --list-adapters and protocol v1). {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            parse_gsusb(&String::from_utf8_lossy(&output.stdout))
        }
        "slcan" => serial_adapters(),
        "pcan" => pcan::adapters(),
        _ => unreachable!(),
    }
    #[cfg(not(windows))]
    {
        let _ = driver;
        let entries = std::fs::read_dir("/sys/class/net").map_err(|e| e.to_string())?;
        Ok(entries
            .filter_map(Result::ok)
            .filter(|entry| {
                std::fs::read_to_string(entry.path().join("type"))
                    .is_ok_and(|kind| kind.trim() == "280")
            })
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                Adapter {
                    device_name: name.clone(),
                    selector: name.clone(),
                    label: name.clone(),
                    identity: format!("socketcan:{name}"),
                    identity_note: String::new(),
                    available: true,
                    reason: String::new(),
                }
            })
            .collect())
    }
}

#[cfg(any(windows, test))]
fn parse_gsusb(output: &str) -> Result<Vec<Adapter>, String> {
    let invalid =
        || "Unrecognized adapter list. Update win-can-utils to a compatible version.".to_owned();
    if output.trim() == "No gs_usb adapters are connected." {
        return Ok(vec![]);
    }
    let mut lines = output.lines().filter(|line| !line.trim().is_empty());
    if lines.next().map(str::trim) != Some("Connected gs_usb adapters:") {
        return Err(invalid());
    }
    let mut adapters = Vec::new();
    for line in lines {
        let (index, row) = line.trim().split_once(") ").ok_or_else(invalid)?;
        index.parse::<usize>().map_err(|_| invalid())?;
        let (location, row) = row.split_once("  ").ok_or_else(invalid)?;
        let (description, status) = row.rsplit_once("  [").ok_or_else(invalid)?;
        let status = status.strip_suffix(']').ok_or_else(invalid)?;
        let interface = location
            .rsplit_once('/')
            .and_then(|(_, n)| n.parse::<u8>().ok());
        let serial = description
            .rsplit_once(" (serial ")
            .and_then(|(_, serial)| serial.strip_suffix(')'))
            .filter(|s| !s.is_empty());
        let selector = match (serial, interface) {
            (Some(serial), Some(interface)) => format!("{serial}/{interface}"),
            _ => location.to_owned(),
        };
        let identity = match (serial, interface) {
            (Some(serial), Some(interface)) => format!("gsusb:{serial}/{interface}"),
            _ => String::new(),
        };
        adapters.push(Adapter {
            device_name: description.rsplit_once(" (serial ").map(|(name, _)| name).unwrap_or(description).to_owned(),
            selector,
            label: format!("{description} — {location}"),
            identity,
            identity_note: if serial.is_some() { "Nickname follows this serial and USB interface across reboots." }
                else { "Serial unavailable. A persistent nickname requires a readable serial; close other servers and refresh." }.into(),
            available: status == "Available",
            reason: if status == "Available" { String::new() } else { status.into() },
        });
    }
    // Duplicated serial/interface pairs are not safe identifiers.
    disambiguate(&mut adapters);
    Ok(adapters)
}

#[cfg(any(windows, test))]
fn disambiguate(adapters: &mut [Adapter]) {
    let mut counts = std::collections::HashMap::new();
    for adapter in adapters.iter().filter(|a| !a.identity.is_empty()) {
        *counts.entry(adapter.identity.clone()).or_insert(0) += 1;
    }
    for adapter in adapters {
        if counts
            .get(&adapter.identity)
            .is_some_and(|&count| count > 1)
        {
            adapter.identity.clear();
            adapter.available = false;
            adapter.reason =
                "Duplicate hardware identity; assign unique device serials/IDs.".into();
            adapter.identity_note = adapter.reason.clone();
        }
    }
}

#[cfg(windows)]
fn serial_adapters() -> Result<Vec<Adapter>, String> {
    use serialport::SerialPortType;
    let mut ports =
        serialport::available_ports().map_err(|e| format!("Cannot list COM ports: {e}"))?;
    ports.sort_by_key(|p| {
        p.port_name
            .trim_start_matches("COM")
            .parse::<u32>()
            .unwrap_or(u32::MAX)
    });
    Ok(ports
        .into_iter()
        .map(|port| {
            let detail = match port.port_type {
                SerialPortType::UsbPort(info) => [info.product, info.serial_number]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" / "),
                SerialPortType::BluetoothPort => "Bluetooth".into(),
                _ => String::new(),
            };
            Adapter {
                device_name: port.port_name.clone(),
                label: if detail.is_empty() {
                    port.port_name.clone()
                } else {
                    format!("{} — {detail}", port.port_name)
                },
                identity: format!("slcan:{}", port.port_name),
                selector: port.port_name,
                identity_note: String::new(),
                available: true,
                reason: String::new(),
            }
        })
        .collect())
}

#[cfg(windows)]
mod pcan {
    use super::*;
    use std::{ffi::c_void, mem::size_of};

    // PCANBasic.h TPCANChannelInformation uses native C alignment (52 bytes).
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Channel {
        handle: u16,
        device_type: u8,
        controller: u8,
        features: u32,
        name: [u8; 33],
        device_id: u32,
        condition: u32,
    }
    type GetValue = unsafe extern "system" fn(u16, u8, *mut c_void, u32) -> u32;

    pub fn adapters() -> Result<Vec<Adapter>, String> {
        // No CAN_Initialize calls: querying the attached channel table does not open a bus.
        let library = unsafe { libloading::Library::new("PCANBasic.dll") }.map_err(|e| {
            format!(
                "Install PEAK PCAN-Basic for this application's architecture (PCANBasic.dll): {e}"
            )
        })?;
        let get =
            unsafe { library.get::<GetValue>(b"CAN_GetValue\0") }.map_err(|e| e.to_string())?;
        let mut count = 0u32;
        check(unsafe { get(0, 0x2A, (&mut count as *mut u32).cast(), 4) })?;
        if count > 1024 {
            return Err("Invalid PCAN attached channel count.".into());
        }
        if count == 0 {
            return Ok(vec![]);
        }
        let mut channels: Vec<Channel> = vec![unsafe { std::mem::zeroed() }; count as usize];
        check(unsafe {
            get(
                0,
                0x2B,
                channels.as_mut_ptr().cast(),
                (channels.len() * size_of::<Channel>()) as u32,
            )
        })?;
        let mut adapters = Vec::new();
        for channel in channels {
            let Some(selector) = channel_name(channel.handle) else {
                continue;
            };
            let mut guid = [0u8; 256];
            let status = unsafe {
                get(
                    channel.handle,
                    0x30,
                    guid.as_mut_ptr().cast(),
                    guid.len() as u32,
                )
            };
            let guid = if status == 0 {
                text(&guid)
            } else {
                String::new()
            };
            let (identity, note) = identity(
                channel.device_type,
                channel.device_id,
                channel.controller,
                &guid,
            );
            adapters.push(Adapter {
                device_name: text(&channel.name),
                selector: selector.clone(),
                label: format!(
                    "{} — {selector} / controller {} / device ID {:X}{}",
                    text(&channel.name),
                    channel.controller + 1,
                    channel.device_id,
                    if guid.is_empty() {
                        String::new()
                    } else {
                        format!(" / {guid}")
                    }
                ),
                identity,
                identity_note: note,
                available: channel.condition & 1 != 0,
                reason: if channel.condition & 1 != 0 {
                    String::new()
                } else {
                    "Channel is in use or unavailable".into()
                },
            });
        }
        disambiguate(&mut adapters);
        Ok(adapters)
    }

    fn check(status: u32) -> Result<(), String> {
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "PCAN discovery failed (0x{status:X}). Update PCAN-Basic and the PEAK driver."
            ))
        }
    }
    fn text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes.split(|&b| b == 0).next().unwrap_or_default())
            .trim()
            .to_owned()
    }

    fn identity(device_type: u8, device_id: u32, controller: u8, guid: &str) -> (String, String) {
        if !guid.is_empty() {
            (
                format!("pcan:guid:{}/{controller}", guid.to_ascii_lowercase()),
                "Nickname follows the hardware GUID and controller across reboots.".into(),
            )
        } else if ![0, 0xFF, u32::MAX].contains(&device_id) {
            (format!("pcan:id:{device_type}:{device_id}/{controller}"), "Nickname follows the configured PCAN device ID and controller. Keep device IDs unique.".into())
        } else {
            (String::new(), "Set a unique device ID with PEAK tools to save nicknames on hardware without GUID support, then refresh.".into())
        }
    }

    fn channel_name(handle: u16) -> Option<String> {
        let (prefix, index) = match handle {
            0x51..=0x58 => ("USB", handle - 0x50),
            0x509..=0x510 => ("USB", handle - 0x500),
            0x41..=0x48 => ("PCI", handle - 0x40),
            0x409..=0x410 => ("PCI", handle - 0x400),
            0x801..=0x810 => ("LAN", handle - 0x800),
            _ => return None,
        };
        Some(format!("{prefix}BUS{index}"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn pcan_layout_handles_and_persistent_identities() {
            assert_eq!(size_of::<Channel>(), 52);
            assert_eq!(channel_name(0x509).as_deref(), Some("USBBUS9"));
            assert_eq!(channel_name(0x410).as_deref(), Some("PCIBUS16"));
            assert_eq!(channel_name(0x801).as_deref(), Some("LANBUS1"));
            assert!(identity(5, 0xFF, 0, "").0.is_empty());
            assert_eq!(identity(5, 42, 1, "").0, "pcan:id:5:42/1");
            assert_eq!(identity(5, 42, 0, "ABC").0, identity(5, 100, 0, "abc").0);
            assert_ne!(identity(5, 42, 0, "ABC").0, identity(5, 42, 1, "ABC").0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gsusb_uses_serial_and_interface_instead_of_usb_location() {
        let a = parse_gsusb("Connected gs_usb adapters:\n  1) 003:062/0  CANable (serial ABC)  [Available]\n  2) 003:063/1  CANable (serial DEF)  [Available]\n").unwrap();
        assert_eq!(a[0].selector, "ABC/0");
        assert_eq!(a[1].identity, "gsusb:DEF/1");
        let b = parse_gsusb(
            "Connected gs_usb adapters:\n  1) 001:002/0  CANable (serial ABC)  [Available]\n",
        )
        .unwrap();
        assert_eq!(a[0].identity, b[0].identity);
        let busy = parse_gsusb("Connected gs_usb adapters:\n  1) 001:002/0  gs_usb adapter  [In use by another server]\n").unwrap();
        assert!(!busy[0].available);
        assert!(busy[0].identity.is_empty());
        assert_eq!(busy[0].selector, "001:002/0");
    }
    #[test]
    fn rejects_unknown_output_and_duplicate_serials() {
        assert!(parse_gsusb("Usage: old canserver.exe").is_err());
        assert!(parse_gsusb("Connected gs_usb adapters:\n malformed").is_err());
        assert!(
            parse_gsusb("No gs_usb adapters are connected.\n")
                .unwrap()
                .is_empty()
        );
        let duplicate = parse_gsusb("Connected gs_usb adapters:\n  1) 001:002/0  CANable (serial ABC)  [Available]\n  2) 001:003/0  CANable (serial ABC)  [Available]\n").unwrap();
        assert!(
            duplicate
                .iter()
                .all(|a| a.identity.is_empty() && !a.available)
        );
    }
}
