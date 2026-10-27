//! Godot-facing worker for offline MF4 browsing. Disk I/O, decompression and
//! optional DBC decoding happen away from the UI and never touch the CAN bus.
use super::reader::Capture;
use crate::can_parser::CanParser;
use godot::classes::RefCounted;
use godot::prelude::*;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};

enum Command {
    Open(String),
    Dbc(String),
    Page {
        group: usize,
        offset: u64,
        limit: usize,
    },
}

struct Worker {
    commands: SyncSender<Command>,
    replies: Receiver<String>,
    cancelled: Arc<AtomicBool>,
}

impl Worker {
    fn new() -> Self {
        let (commands, input) = mpsc::sync_channel(1);
        let (output, replies) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellation = Arc::clone(&cancelled);
        std::thread::spawn(move || run(input, output, cancellation));
        Self {
            commands,
            replies,
            cancelled,
        }
    }

    fn send(&self, command: Command) -> String {
        self.commands
            .try_send(command)
            .err()
            .map(|e| format!("MF4 reader is busy or unavailable: {e}"))
            .unwrap_or_default()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

fn run(input: Receiver<Command>, output: SyncSender<String>, cancelled: Arc<AtomicBool>) {
    let mut capture: Option<Capture> = None;
    let mut parser = CanParser::new();
    while let Ok(command) = input.recv() {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        let result: Result<Value, String> = match command {
            Command::Open(path) => Capture::open(Path::new(&path), &cancelled).map(|opened| {
                let result = json!({"kind": "opened", "path": path, "info": opened.info()});
                capture = Some(opened);
                result
            }),
            Command::Dbc(path) => {
                let mut candidate = CanParser::new();
                let result = if path.is_empty() {
                    Ok(())
                } else {
                    candidate
                        .open_dbc(path.clone())
                        .map_err(|e| format!("Cannot load DBC: {e:?}"))
                };
                result.map(|()| {
                    parser = candidate;
                    json!({"kind": "dbc", "path": path})
                })
            }
            Command::Page {
                group,
                offset,
                limit,
            } => capture
                .as_mut()
                .ok_or("Open an MF4 file first.".into())
                .and_then(|capture| capture.page(group, offset, limit, &parser, &cancelled))
                .map(|page| json!({"kind": "page", "page": page})),
        };
        let reply = result.unwrap_or_else(|error| json!({"kind": "error", "error": error}));
        if output.send(reply.to_string()).is_err() {
            break;
        }
    }
}

#[derive(GodotClass)]
#[class(base=RefCounted)]
struct Mf4Decoder {
    worker: Option<Worker>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for Mf4Decoder {
    fn init(base: Base<RefCounted>) -> Self {
        Self { worker: None, base }
    }
}

#[godot_api]
impl Mf4Decoder {
    #[func]
    fn open_capture(&mut self, path: String) -> String {
        self.worker
            .get_or_insert_with(Worker::new)
            .send(Command::Open(path))
    }

    #[func]
    fn set_dbc(&mut self, path: String) -> String {
        self.worker
            .get_or_insert_with(Worker::new)
            .send(Command::Dbc(path))
    }

    #[func]
    fn request_page(&mut self, group: i64, offset: i64, limit: i32) -> String {
        if group < 0 || offset < 0 || !(1..=500).contains(&limit) {
            return "Invalid MF4 page request.".into();
        }
        self.worker
            .get_or_insert_with(Worker::new)
            .send(Command::Page {
                group: group as usize,
                offset: offset as u64,
                limit: limit as usize,
            })
    }

    #[func]
    fn poll(&mut self) -> String {
        self.worker
            .as_ref()
            .and_then(|worker| worker.replies.try_recv().ok())
            .unwrap_or_default()
    }

    #[func]
    fn close_capture(&mut self) {
        self.worker.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn request(worker: &Worker, command: Command) -> Value {
        assert!(worker.send(command).is_empty());
        serde_json::from_str(&worker.replies.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap()
    }

    #[test]
    fn bad_file_or_dbc_keeps_the_previous_capture_and_decoder() {
        let worker = Worker::new();
        let path = Path::new("../godot/tests/mf4/raw.mf4")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            request(&worker, Command::Open(path.clone()))["kind"],
            "opened"
        );
        assert_eq!(
            request(&worker, Command::Dbc("../godot/tests/transmit.dbc".into()))["kind"],
            "dbc"
        );
        assert_eq!(
            request(&worker, Command::Open("missing/capture.mf4".into()))["kind"],
            "error"
        );
        assert_eq!(request(&worker, Command::Dbc(path))["kind"], "error");
        let page = request(
            &worker,
            Command::Page {
                group: 0,
                offset: 0,
                limit: 1,
            },
        );
        assert!(
            page["page"]["rows"][0][6]
                .as_str()
                .unwrap()
                .contains("SetComplementaryFilter.odr = 11")
        );
        assert_eq!(request(&worker, Command::Dbc("".into()))["kind"], "dbc");
        let page = request(
            &worker,
            Command::Page {
                group: 0,
                offset: 0,
                limit: 1,
            },
        );
        assert_eq!(page["page"]["rows"][0][6], "");
    }
}
