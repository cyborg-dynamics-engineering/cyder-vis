//! Observe running interfaces without taking the UI's exclusive active connection.
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::{runtime::Runtime, task::JoinHandle};

#[derive(Clone, Serialize)]
pub struct RunningInterface {
    pub name: String,
    pub driver: String,
    pub device: String,
    pub bitrate: u32,
    pub bus_load: Option<f64>,
    pub state: String,
    pub error: String,
}

type Snapshot = Arc<Mutex<HashMap<String, RunningInterface>>>;

#[derive(Default)]
pub struct InterfaceRegistry {
    snapshot: Snapshot,
    task: Option<JoinHandle<()>>,
}

impl InterfaceRegistry {
    pub fn snapshot(&mut self, runtime: &Runtime) -> Vec<RunningInterface> {
        if self.task.is_none() {
            self.task = Some(runtime.spawn(observe(self.snapshot.clone())));
        }
        let mut interfaces: Vec<_> = self.snapshot.lock().unwrap().values().cloned().collect();
        interfaces.sort_by(|a, b| a.name.cmp(&b.name));
        interfaces
    }
}

impl Drop for InterfaceRegistry {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn observe(snapshot: Snapshot) {
    let mut tasks = tokio::task::JoinSet::new();
    let mut active = std::collections::HashSet::new();
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = timer.tick() => {
                for name in running_names() {
                    if active.insert(name.clone()) {
                        let snapshot = snapshot.clone();
                        tasks.spawn(async move {
                            monitor(&name, snapshot.clone()).await;
                            snapshot.lock().unwrap().remove(&name);
                            name
                        });
                    }
                }
            }
            Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                if let Ok(name) = result { active.remove(&name); }
            }
        }
    }
}

#[cfg(windows)]
fn running_names() -> Vec<String> {
    std::fs::read_dir(r"\\.\pipe\")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file = entry.file_name();
            let name = file
                .to_str()?
                .strip_prefix("wincan_")?
                .strip_suffix("_v1")?;
            crosscan::protocol::validate_name(name).ok()?;
            Some(name.to_owned())
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn running_names() -> Vec<String> {
    std::fs::read_dir("/sys/class/net")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            std::fs::read_to_string(entry.path().join("type"))
                .is_ok_and(|kind| kind.trim() == "280")
        })
        .filter(|entry| {
            std::fs::read_to_string(entry.path().join("flags"))
                .ok()
                .and_then(|flags| {
                    u32::from_str_radix(flags.trim().trim_start_matches("0x"), 16).ok()
                })
                .is_some_and(|flags| flags & 1 != 0)
        })
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

#[cfg(windows)]
async fn monitor(name: &str, snapshot: Snapshot) {
    use crosscan::{protocol::*, service::ServiceClient};
    let Ok(mut client) = ServiceClient::connect(
        name,
        Subscription {
            error_mask: 0x1fff_ffff,
            ..Default::default()
        },
    )
    .await
    else {
        return;
    };
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
    let mut period = Instant::now();
    let mut bits = 0u64;
    loop {
        tokio::select! {
            event = client.next_event() => {
                let Ok(event) = event else { return; };
                if let Ok(frame) = event.frame.to_can() { bits += crate::can_frame_bits(&frame) as u64; }
            }
            _ = timer.tick() => {
                let Ok(Ok(Reply::Status { status })) = tokio::time::timeout(std::time::Duration::from_secs(2), client.request(Command::Status)).await else { return; };
                let elapsed = period.elapsed().as_secs_f64();
                let load = (elapsed >= 0.5 && status.state == InterfaceState::Up && status.bitrate > 0)
                    .then(|| (100.0 * bits as f64 / elapsed / status.bitrate as f64).clamp(0.0, 100.0));
                snapshot.lock().unwrap().insert(name.into(), RunningInterface {
                    name: name.into(), driver: status.driver, device: status.device,
                    bitrate: status.bitrate, bus_load: load,
                    state: format!("{:?}", status.state).to_lowercase(), error: status.last_error.unwrap_or_default(),
                });
                bits = 0;
                period = Instant::now();
            }
        }
    }
}

#[cfg(target_os = "linux")]
async fn monitor(name: &str, snapshot: Snapshot) {
    use crosscan::{CanInterface, lin_can::LinuxCan};
    let Ok(mut socket) = LinuxCan::open(name).await else {
        return;
    };
    let bitrate = socket
        .get_bitrate()
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
    let mut period = Instant::now();
    let mut bits = 0u64;
    loop {
        tokio::select! {
            frame = socket.read_frame() => {
                let Ok(frame) = frame else { return; };
                bits += crate::can_frame_bits(&frame) as u64;
            }
            _ = timer.tick() => {
                if !running_names().iter().any(|candidate| candidate == name) { return; }
                let elapsed = period.elapsed().as_secs_f64();
                snapshot.lock().unwrap().insert(name.into(), RunningInterface {
                    name: name.into(), driver: "socketcan".into(), device: name.into(), bitrate,
                    bus_load: (elapsed >= 0.5 && bitrate > 0).then(|| (100.0 * bits as f64 / elapsed / bitrate as f64).clamp(0.0, 100.0)),
                    state: "up".into(), error: String::new(),
                });
                bits = 0; period = Instant::now();
            }
        }
    }
}

/// Pipe names are limited by crosscan; the UI retains the full display name.
pub fn service_name(display: &str, identity: &str, occupied: &[String]) -> String {
    let base: String = display
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    let base: String = base.chars().take(48).collect();
    let base = if base.is_empty() {
        "CAN".to_owned()
    } else {
        base
    };
    if !occupied.contains(&base) {
        return base;
    }
    let hash = identity.bytes().fold(2166136261u32, |hash, byte| {
        (hash ^ byte as u32).wrapping_mul(16777619)
    });
    let candidate = format!("{base}_{hash:08x}");
    if !occupied.contains(&candidate) {
        return candidate;
    }
    (2..)
        .map(|index| format!("{base}_{index}"))
        .find(|name| !occupied.contains(name))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_follow_display_names_and_handle_collisions() {
        assert_eq!(
            service_name("Powertrain bench", "serial-a", &[]),
            "Powertrain_bench"
        );
        assert_eq!(
            service_name("Cyder SH-C31A gs_usb", "serial-a", &[]),
            "Cyder_SH-C31A_gs_usb"
        );
        let names = vec!["Powertrain_bench".into()];
        assert_ne!(
            service_name("Powertrain bench", "serial-a", &names),
            service_name("Powertrain bench", "serial-b", &names)
        );
        crosscan::protocol::validate_name(&service_name(&"x".repeat(200), "serial", &[])).unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn finds_running_service_pipes_without_a_saved_configuration() {
        let name = format!("cv_registry_test_{}", std::process::id());
        let _pipe = tokio::net::windows::named_pipe::ServerOptions::new()
            .first_pipe_instance(true)
            .create(crosscan::protocol::pipe_name(&name).unwrap())
            .unwrap();
        assert!(running_names().contains(&name));
    }
}
