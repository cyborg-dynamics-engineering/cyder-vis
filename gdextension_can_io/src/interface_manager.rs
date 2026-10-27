use crate::system_tools::find_in_path;
use std::process::{Child, Command, Stdio};
use std::thread::JoinHandle;

#[derive(Default)]
pub struct InterfaceManager {
    job: Option<JoinHandle<Result<(), String>>>,
    server: Option<Child>,
    server_log: String,
}

pub fn linux_commands(
    interface: &str,
    bitrate: u32,
    virtual_bus: bool,
    create: bool,
) -> Result<Vec<Vec<String>>, String> {
    validate(interface, bitrate)?;
    let commands: Vec<Vec<&str>> = if create {
        if !virtual_bus {
            return Err("Physical CAN adapters must be connected before bringing them up.".into());
        }
        vec![
            vec!["link", "add", "dev", interface, "type", "vcan"],
            vec!["link", "set", "dev", interface, "up"],
        ]
    } else if virtual_bus {
        vec![
            vec!["link", "set", "dev", interface, "down"],
            vec!["link", "set", "dev", interface, "up"],
        ]
    } else {
        let rate = bitrate.to_string();
        return Ok(vec![
            vec!["link", "set", "dev", interface, "down"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            vec![
                "link", "set", "dev", interface, "type", "can", "bitrate", &rate,
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            vec!["link", "set", "dev", interface, "up"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ]);
    };
    Ok(commands
        .into_iter()
        .map(|args| args.into_iter().map(str::to_owned).collect())
        .collect())
}

fn validate(interface: &str, bitrate: u32) -> Result<(), String> {
    if interface.is_empty()
        || interface.len() > 15
        || !interface
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_- .".contains(&c) && c != b' ')
    {
        return Err(
            "Use an interface name of 1–15 letters, numbers, underscores, dots or hyphens.".into(),
        );
    }
    if bitrate == 0 || bitrate > 1_000_000 {
        return Err("Classic CAN bitrate must be between 1 and 1000000 bit/s.".into());
    }
    Ok(())
}

impl InterfaceManager {
    pub fn start(
        &mut self,
        interface: &str,
        bitrate: u32,
        virtual_bus: bool,
        create: bool,
        driver: &str,
        channel: &str,
        log: &str,
    ) -> Result<(), String> {
        validate(interface, bitrate)?;
        if self.job.is_some() {
            return Err("An interface operation is already in progress.".into());
        }
        let search = std::env::var_os("PATH").unwrap_or_default();
        if cfg!(target_os = "linux") {
            let ip =
                find_in_path("ip", &search).ok_or("Install iproute2 and ensure ip is on PATH.")?;
            let pkexec = find_in_path("pkexec", &search)
                .ok_or("Install polkit (pkexec) and enable a desktop authentication agent.")?;
            let commands = linux_commands(interface, bitrate, virtual_bus, create)?;
            self.job = Some(std::thread::spawn(move || {
                for args in commands {
                    let output = Command::new(&pkexec)
                        .arg("--disable-internal-agent")
                        .arg(&ip)
                        .args(&args)
                        .output()
                        .map_err(|e| e.to_string())?;
                    if !output.status.success() {
                        return Err(format!(
                            "Interface operation failed ({}): {}. Check authorization and that the device exists.",
                            output.status,
                            String::from_utf8_lossy(&output.stderr)
                        ));
                    }
                }
                Ok(())
            }));
        } else if cfg!(target_os = "windows") {
            if !["gsusb", "slcan", "pcan"].contains(&driver) {
                return Err("Select gsusb, slcan or pcan.".into());
            }
            let executable = find_in_path("canserver.exe", &search)
                .ok_or("Install win-can-utils and put canserver.exe on PATH.")?;
            self.stop_server()?;
            let output = std::fs::File::create(log)
                .map_err(|e| format!("Cannot open canserver log: {e}"))?;
            let mut command = Command::new(executable);
            command.args([
                driver,
                "--bitrate",
                &bitrate.to_string(),
                "--name",
                interface,
            ]);
            if !channel.trim().is_empty() {
                command.args(["--channel", channel.trim()]);
            }
            command
                .stdin(Stdio::null())
                .stdout(output.try_clone().map_err(|e| e.to_string())?)
                .stderr(output);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            self.server = Some(command.spawn().map_err(|e| e.to_string())?);
            self.server_log = log.to_owned();
        } else {
            return Err("Interface management is supported on Windows and Linux.".into());
        }
        Ok(())
    }

    /// Empty while idle/running; completion and early server exits are reported to the UI.
    pub fn poll(&mut self) -> String {
        if self.job.as_ref().is_some_and(|job| job.is_finished()) {
            return match self.job.take().unwrap().join() {
                Ok(Ok(())) => "Interface is up. Click Start to connect.".into(),
                Ok(Err(error)) => format!("ERROR: {error}"),
                Err(_) => "ERROR: Interface operation failed unexpectedly.".into(),
            };
        }
        if let Some(server) = &mut self.server {
            match server.try_wait() {
                Ok(Some(status)) => {
                    self.server = None;
                    return format!(
                        "ERROR: canserver exited ({status}). See {}. Check adapter selection, installed win-can-utils version and driver.",
                        self.server_log
                    );
                }
                Err(error) => return format!("ERROR: Cannot check canserver: {error}"),
                _ => {}
            }
        }
        String::new()
    }

    pub fn stop_server(&mut self) -> Result<(), String> {
        if let Some(mut server) = self.server.take() {
            if server.try_wait().map_err(|e| e.to_string())?.is_none() {
                server.kill().map_err(|e| e.to_string())?;
            }
            server.wait().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

impl Drop for InterfaceManager {
    fn drop(&mut self) {
        let _ = self.stop_server();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_physical_restart_and_virtual_creation() {
        let commands = linux_commands("can0", 500_000, false, false).unwrap();
        assert_eq!(commands[0], ["link", "set", "dev", "can0", "down"]);
        assert_eq!(
            commands[1],
            [
                "link", "set", "dev", "can0", "type", "can", "bitrate", "500000"
            ]
        );
        assert_eq!(commands[2], ["link", "set", "dev", "can0", "up"]);
        assert_eq!(
            linux_commands("vcan0", 1, true, true).unwrap()[0],
            ["link", "add", "dev", "vcan0", "type", "vcan"]
        );
        assert_eq!(linux_commands("vcan0", 1, true, false).unwrap().len(), 2);
    }

    #[test]
    fn rejects_invalid_interfaces_and_rates() {
        for name in [
            "",
            "can0;reboot",
            "a b",
            "../../can0",
            "this_name_is_too_long",
        ] {
            assert!(linux_commands(name, 500_000, false, false).is_err());
        }
        assert!(linux_commands("can0", 0, false, false).is_err());
        assert!(linux_commands("can0", 1_000_001, false, false).is_err());
        assert!(linux_commands("can0", 500_000, false, true).is_err());
    }
}
