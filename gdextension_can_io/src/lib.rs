mod can_parser;
mod dbc_transmit;
mod interface_manager;
mod recording;
mod system_tools;
mod trace_filter;

use crate::can_parser::CanParser;
use crate::trace_filter::TraceFilter;
use crosscan::CanInterface;
use crosscan::can::CanFrame;
use godot::classes::{Node, ResourceLoader, Script};
use godot::prelude::*;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

struct CanGDExtension;

// Assigns this file as the GDExtension entry-point
#[gdextension]
unsafe impl ExtensionLibrary for CanGDExtension {}

// Implement the GodotCanBridge as a Node for use inside Godot
#[derive(GodotClass)]
#[class(base=Node)]
struct GodotCanBridge {
    /// Abstracted tool for parsing semantic data from a CAN frame (using a DBC file if provided)
    can_parser: CanParser,
    /// A thread handle for the CAN 'read' task
    read_handle: Option<tokio::task::JoinHandle<()>>,
    /// The OS interface handle of the CAN bus
    interface: String,
    /// The bitrate of the CAN bus
    bitrate: Arc<Mutex<u32>>,
    /// Holds the estimated number of bits sent over the bus. Should be reset every time its read
    bit_counter: Arc<Mutex<usize>>,
    /// A table containing the parsed CAN messages as entries for a given CAN ID. New messages with a given CAN ID should replace the existing data in the old entry
    can_entries: Arc<Mutex<HashMap<CanId, CanEntry>>>,
    /// Queue of CanFrames that have been requested to send. Messages should be removed from the queue once sent
    sending_queue: Arc<Mutex<VecDeque<CanFrame>>>,
    /// True if the CanBridge has been requested to terminate
    closure_requested: Arc<Mutex<bool>>,
    /// The threading runtime environment
    runtime: tokio::runtime::Runtime,
    /// The time that the CAN bridge started running
    start_time: Arc<Mutex<Instant>>,
    /// Holds the recent can frames with timestamps for trace
    trace_history: Arc<Mutex<VecDeque<CanFrame>>>,
    trace_filter: Arc<Mutex<TraceFilter>>,
    interface_manager: interface_manager::InterfaceManager,
    recorder: Arc<Mutex<Option<recording::Recorder>>>,

    /// Base Godot class (Rust module will be implemented as a Node in Godot)
    base: Base<Node>,
}

struct CanEntry {
    timestamps: VecDeque<u128>,
    last_timestamp: u128,
    freq_hz: f32,
    frame: CanFrame,
}

type CanId = u32;

#[godot_api]
impl INode for GodotCanBridge {
    // 'init' is called in Godot everytime the extension is loaded into the editor
    fn init(base: Base<Node>) -> Self {
        godot_print!("RUST CAN module loaded");

        Self {
            can_parser: CanParser::new(),
            read_handle: None,
            interface: "".to_string(),
            bitrate: Arc::new(Mutex::new(0)),
            bit_counter: Arc::new(Mutex::new(0)),
            can_entries: Arc::new(Mutex::new(HashMap::<CanId, CanEntry>::new())),
            sending_queue: Arc::new(Mutex::new(VecDeque::<CanFrame>::new())),
            closure_requested: Arc::new(Mutex::new(false)),
            runtime: Runtime::new().unwrap(),
            start_time: Arc::new(Mutex::new(Instant::now())),
            trace_history: Arc::new(Mutex::new(VecDeque::new())),
            trace_filter: Arc::new(Mutex::new(TraceFilter::default())),
            interface_manager: interface_manager::InterfaceManager::default(),
            recorder: Arc::new(Mutex::new(None)),
            base,
        }
    }
}

#[godot_api]
impl GodotCanBridge {
    #[func]
    fn start_recording(&mut self, directory: String, mode: i32, format: String) -> String {
        if !self.is_alive() {
            return "Connect to a CAN interface before starting a recording.".into();
        }
        let mut recorder = self.runtime.block_on(self.recorder.lock());
        if recorder.is_some() {
            return "A recording is already running.".into();
        }
        match recording::Recorder::start(
            std::path::Path::new(&directory),
            mode,
            &format,
            self.can_parser.clone(),
        ) {
            Ok(value) => {
                *recorder = Some(value);
                String::new()
            }
            Err(error) => error,
        }
    }

    #[func]
    fn stop_recording(&mut self) -> String {
        if let Some(mut recorder) = self.runtime.block_on(self.recorder.lock()).take() {
            return recorder.finish().err().unwrap_or_default();
        }
        String::new()
    }

    #[func]
    fn is_recording(&self) -> bool {
        self.runtime.block_on(self.recorder.lock()).is_some()
    }

    #[func]
    fn recording_files(&self) -> String {
        self.runtime
            .block_on(self.recorder.lock())
            .as_ref()
            .map(|recorder| {
                recorder
                    .paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }

    #[func]
    fn dbc_transmit_definitions(&self) -> String {
        self.can_parser
            .dbc()
            .map(|dbc| serde_json::to_string(&dbc_transmit::definitions(dbc)).unwrap())
            .unwrap_or_else(|| "[]".into())
    }

    #[func]
    fn encode_dbc_frame(&self, id: u32, extended: bool, inputs_json: String) -> String {
        let result = (|| {
            let dbc = self.can_parser.dbc().ok_or("Load a DBC file first")?;
            let inputs: Vec<String> =
                serde_json::from_str(&inputs_json).map_err(|e| e.to_string())?;
            dbc_transmit::encode(dbc, id, extended, &inputs)
        })();
        match result {
            Ok(data) => serde_json::json!({"data": data}).to_string(),
            Err(error) => serde_json::json!({"error": error}).to_string(),
        }
    }

    #[func]
    fn check_dependencies(&self) -> String {
        system_tools::can_utils_warning()
    }

    #[func]
    fn manage_interface(
        &mut self,
        interface: String,
        bitrate: u32,
        virtual_bus: bool,
        create: bool,
        driver: String,
        channel: String,
        log: String,
    ) -> String {
        if self.is_alive() {
            return "Pause the CAN connection before changing the interface.".into();
        }
        self.interface_manager
            .start(
                &interface,
                bitrate,
                virtual_bus,
                create,
                &driver,
                &channel,
                &log,
            )
            .err()
            .unwrap_or_default()
    }

    #[func]
    fn interface_operation_status(&mut self) -> String {
        self.interface_manager.poll()
    }

    #[func]
    fn stop_managed_server(&mut self) -> String {
        if self.is_alive() {
            return "Pause the CAN connection before stopping its server.".into();
        }
        self.interface_manager
            .stop_server()
            .err()
            .unwrap_or_default()
    }

    #[func]
    fn load_dbc_file(&mut self, dbc_filepath: String) -> bool {
        // Empty filepath, assume user does not have a DBC file
        if dbc_filepath.trim().is_empty() {
            self.can_parser.clear_dbc();
            return true;
        }

        match self.can_parser.open_dbc(dbc_filepath.clone()) {
            Ok(_) => {
                godot_print!("Loaded DBC file from [{:?}]", dbc_filepath);
                true
            }
            Err(e) => {
                match e {
                    can_parser::Error::Io(error) => {
                        error_alert_godot(format!(
                            "Error trying to open DBC file at {dbc_filepath:?}"
                        ));
                        godot_error!("{error:?}");
                    }
                    can_parser::Error::CanDbc(error) => error_alert_godot(format!(
                        "DBC File failed to parse: \n{}",
                        error.to_string()
                    )),
                }
                false
            }
        }
    }

    #[func]
    fn configure_bus(&mut self, interface_name: String) -> bool {
        let warning = self.check_dependencies();
        if !warning.is_empty() {
            error_alert_godot(warning);
            return false;
        }
        // Check if multithreading is functional in this godot-rust version
        if let Err(err) = std::thread::spawn(|| {
            godot_print!("RUST threading successful");
        })
        .join()
        {
            let msg = match err.downcast_ref::<&'static str>() {
                Some(s) => *s,
                None => match err.downcast_ref::<String>() {
                    Some(s) => &s[..],
                    None => "Sorry, unknown payload type",
                },
            };
            error_alert_godot(format!("Error when attempting to thread: {:?}", msg));
        }

        self.interface = interface_name.clone();

        // Create the CAN read/write thread
        let _guard = self.runtime.enter();
        let bitrate = Arc::clone(&self.bitrate);
        let bit_counter = Arc::clone(&self.bit_counter);
        let can_entries = Arc::clone(&self.can_entries);
        let sending_queue = Arc::clone(&self.sending_queue);
        let closure_requested = Arc::clone(&self.closure_requested);
        let start_time = Arc::clone(&self.start_time);
        let trace_history = Arc::clone(&self.trace_history);
        let trace_filter = Arc::clone(&self.trace_filter);
        let recorder = Arc::clone(&self.recorder);
        self.read_handle = Some(tokio::spawn(async {
            read_can(
                interface_name,
                bitrate,
                bit_counter,
                can_entries,
                sending_queue,
                closure_requested,
                start_time,
                trace_history,
                trace_filter,
                recorder,
            )
            .await;
        }));

        godot_print!("CAN bus opened");
        return true;
    }

    #[func]
    fn get_can_table(&mut self) -> VariantArray {
        self.can_parser
            .parse_can_table(&self.runtime.block_on(self.can_entries.lock()))
    }

    #[func]
    fn clear_can_table(&mut self) {
        self.runtime.block_on(self.can_entries.lock()).clear();
    }

    #[func]
    fn clear_can_entry(&mut self, can_id_value: u32) {
        self.runtime
            .block_on(self.can_entries.lock())
            .remove_entry(&can_id_value);
    }

    #[func]
    fn send_can_frame(&mut self, can_id_value: u32, is_extended: bool, data: VariantArray) {
        // Convert from Godot Variant to typed u8 vector
        let packed_bytes = PackedByteArray::from(&data);
        let byte_slice_data: &[u8] = packed_bytes.as_slice();

        // Create a CAN data frame with the ID and some data (up to 8 bytes for a standard CAN frame)
        let frame = if is_extended {
            CanFrame::new_eff(can_id_value, &byte_slice_data).unwrap()
        } else {
            CanFrame::new(can_id_value, &byte_slice_data).unwrap()
        };

        self.runtime
            .block_on(self.sending_queue.lock())
            .push_back(frame);
    }

    #[func]
    fn close_bus(&mut self) {
        if let Some(handle) = self.read_handle.take() {
            // Flag the thread to end
            *self.runtime.block_on(self.closure_requested.lock()) = true;

            // Wait for thread to complete
            self.runtime.block_on(handle).unwrap();

            godot_print!("CAN bus closed");
        } else {
            error_alert_godot("Attempted to close a nonexistent CAN connection".to_string());
        }
        let error = self.stop_recording();
        if !error.is_empty() {
            error_alert_godot(format!("Recording finalization failed: {error}"));
        }
    }

    #[func]
    fn is_alive(&mut self) -> bool {
        if let Some(handle) = &self.read_handle {
            return !handle.is_finished();
        }
        false
    }

    #[func]
    fn get_interface(&mut self) -> String {
        if self.is_alive() {
            return self.interface.clone();
        }
        "".to_string()
    }

    #[func]
    fn get_bitrate(&mut self) -> u32 {
        if self.is_alive() {
            return *self.runtime.block_on(self.bitrate.lock());
        }
        0
    }

    /// Returns the number of bits sent/received over the bus since the last time this function was called
    #[func]
    fn get_bus_bits(&mut self) -> u64 {
        if self.is_alive() {
            let mut bit_counter_mutex = self.runtime.block_on(self.bit_counter.lock());
            let count = (*bit_counter_mutex).try_into().unwrap();

            // Reset the counter before returning the old count value
            *bit_counter_mutex = 0;
            return count;
        }
        0
    }

    #[func]
    fn set_trace_filter(&mut self, input: String) -> String {
        match TraceFilter::parse(&input) {
            Ok(filter) => {
                *self.runtime.block_on(self.trace_filter.lock()) = filter;
                String::new()
            }
            Err(error) => error,
        }
    }

    #[func]
    fn get_recent_can_msgs(&mut self) -> GString {
        {
            let trace_history_lock = self.runtime.block_on(self.trace_history.lock());
            let filter = self.runtime.block_on(self.trace_filter.lock());
            let mut gd_can_history = String::new();
            for frame in trace_history_lock.iter() {
                if !filter.matches(frame, &self.interface) {
                    continue;
                }
                // Push the raw CAN frame as a formatted String
                gd_can_history.push_str(&format_can_frame(frame));
                gd_can_history.push_str("\n");

                // Push the deserialised CAN data as a String
                if let Some(parsed_data) = self.can_parser.parse_frame_as_string(frame) {
                    gd_can_history.push_str(&parsed_data);
                }
            }
            return GString::from(gd_can_history);
        }
    }
}

async fn read_can(
    interface_name: String,
    bitrate: Arc<Mutex<u32>>,
    bit_counter: Arc<Mutex<usize>>,
    can_entries: Arc<Mutex<HashMap<CanId, CanEntry>>>,
    sending_queue: Arc<Mutex<VecDeque<CanFrame>>>,
    closure_requested: Arc<Mutex<bool>>,
    start_time: Arc<Mutex<Instant>>,
    trace_history: Arc<Mutex<VecDeque<CanFrame>>>,
    trace_filter: Arc<Mutex<TraceFilter>>,
    recorder: Arc<Mutex<Option<recording::Recorder>>>,
) {
    // Select a specific CAN Socket implementation for the supported operating systems
    #[cfg(target_os = "linux")]
    use crosscan::lin_can::LinuxCan as CanSocket;

    #[cfg(target_os = "windows")]
    use crosscan::win_can::WindowsCan as CanSocket;

    // Open async CAN socket
    let mut socket = match CanSocket::open(&interface_name).await {
        Ok(sock) => sock,
        Err(err) => {
            error_alert_godot(format!(
                "Failed to open CAN device, check that {interface_name:?} is up"
            ));
            godot_error!("{err:?}");
            finish_recording(&recorder).await;
            return;
        }
    };

    // Save the bitrate
    *bitrate.lock().await = match socket.get_bitrate().await {
        Ok(br) => br.unwrap_or(0),
        Err(err) => {
            error_alert_godot(format!("Failed to read bitrate on {interface_name:?}"));
            godot_error!("{err:?}");
            0
        }
    };

    loop {
        {
            let mut recording = recorder.lock().await;
            if let Some(active) = recording.as_mut() {
                if let Err(error) = active.flush_if_due() {
                    recording.take();
                    error_alert_godot(format!("Recording stopped: {error}"));
                }
            }
        }
        // Process outgoing CAN messages
        {
            let mut frames_to_send = sending_queue.lock().await;
            for frame in frames_to_send.clone().into_iter() {
                *bit_counter.lock().await += can_frame_bits(&frame);

                if let Err(e) = socket.write_frame(frame).await {
                    error_alert_godot(format!(
                        "Error when transmitting frames (Some messages may have not been sent): {:?}",
                        e.to_string()
                    ));
                    godot_error!("{e:?}");
                    break;
                }
            }
            frames_to_send.clear();
        }

        // Check if the bus should be closed
        {
            let mut should_close = closure_requested.lock().await;
            if *should_close {
                *should_close = false;

                // Breaks out of the loop, ending the thread
                break;
            }
        }

        // Read a CanFrame. If none found within timeout, continue the loop (rechecks for outgoing or closing requests then try again)
        let res = match tokio::time::timeout(Duration::from_millis(100), socket.read_frame()).await
        {
            Ok(res) => res,
            Err(_) => continue,
        };

        // Process the incoming CanFrame
        match res {
            Ok(frame) => {
                *bit_counter.lock().await += can_frame_bits(&frame);

                let current_timestamp_us = { start_time.lock().await.elapsed().as_micros() };

                // Add to the recent CAN history
                let mut frame_with_timestamp = frame.clone();
                frame_with_timestamp.set_timestamp(Some(current_timestamp_us.try_into().unwrap()));

                {
                    let filter = trace_filter.lock().await.clone();
                    let mut recording = recorder.lock().await;
                    if let Some(active) = recording.as_mut() {
                        if let Err(error) =
                            active.record(&frame_with_timestamp, &interface_name, &filter)
                        {
                            recording.take();
                            error_alert_godot(format!("Recording stopped: {error}"));
                        }
                    }
                }

                let mut trace_history_lock = trace_history.lock().await;
                trace_history_lock.push_back(frame_with_timestamp);
                const MAX_TRACE_LENGTH: usize = 100;
                if trace_history_lock.len() > MAX_TRACE_LENGTH {
                    trace_history_lock.pop_front();
                }

                // Check if this msg's CAN ID is in the CanEntry table
                let mut can_entries = can_entries.lock().await;
                match can_entries.entry(frame.id()) {
                    Entry::Occupied(mut occupied_entry) => {
                        let can_entry = occupied_entry.get_mut();

                        // push new timestamp
                        can_entry.timestamps.push_back(current_timestamp_us);

                        // drop old (>100ms)
                        while let Some(&front) = can_entry.timestamps.front() {
                            if current_timestamp_us - front > 100_000 {
                                can_entry.timestamps.pop_front();
                            } else {
                                break;
                            }
                        }

                        // default: window count if dense
                        let mut freq_hz = if can_entry.timestamps.len() > 1 {
                            (can_entry.timestamps.len() as f32) * 10.0
                        } else {
                            can_entry.freq_hz
                        };

                        // always use direct delta if >50ms gap
                        let delta_us = current_timestamp_us - can_entry.last_timestamp;
                        if delta_us > 50_000 {
                            freq_hz = 1e6 / (delta_us as f32);
                        }

                        // -------- Exponential moving average filter --------
                        let alpha = (delta_us as f32 / 1e6).clamp(0.003, 1.0);
                        can_entry.freq_hz = alpha * freq_hz + (1.0 - alpha) * can_entry.freq_hz;

                        // Update
                        can_entry.last_timestamp = current_timestamp_us;
                        can_entry.frame = frame;
                    }

                    Entry::Vacant(entry) => {
                        let mut timestamps = VecDeque::new();
                        timestamps.push_back(current_timestamp_us);

                        entry.insert(CanEntry {
                            timestamps,
                            last_timestamp: current_timestamp_us,
                            freq_hz: 0.0,
                            frame,
                        });
                    }
                }
            }
            Err(err) => {
                error_alert_godot(format!("Received CAN error: {:?}", err));
                break;
            }
        }
    }
    finish_recording(&recorder).await;
}

async fn finish_recording(recorder: &Mutex<Option<recording::Recorder>>) {
    if let Some(mut active) = recorder.lock().await.take() {
        if let Err(error) = active.finish() {
            error_alert_godot(format!("Recording finalization failed: {error}"));
        }
    }
}

/// Sends an error popup to the user in Godot and logs the error to the Godot standard output
fn error_alert_godot(msg: String) {
    let mut script = ResourceLoader::singleton()
        .load("res://assets/alert_handler.gd")
        .unwrap()
        .cast::<Script>();

    let args = &[msg.to_variant()];
    script.call_deferred("display_error", args);

    godot_error!("{:?}", msg);
}

/// Returns the size in bytes of a given CanFrame when it is on the CAN bus
fn can_frame_bits(frame: &CanFrame) -> usize {
    let base = if frame.is_extended() { 67 } else { 47 }; // All non-data fields
    let data_bits = frame.dlc() * 8;
    let est_stuff = ((base + data_bits) as f32 * 0.2) as usize; // Estimate 20% bit stuffing on average

    base + data_bits + est_stuff
}

/// Converts a CanFrame into a String representation
fn format_can_frame(frame: &CanFrame) -> String {
    let mut frame_string = String::new();

    let us: u64 = match frame.timestamp() {
        Some(timestamp) => timestamp,
        None => 0,
    };
    frame_string.push_str(&format!("{:04}.{:06}  ", us / 1_000_000, us % 1_000_000));

    if frame.is_extended() {
        frame_string.push_str(&format!("{:08X}   ", frame.id()));
    } else {
        frame_string.push_str(&format!("{:03X}   ", frame.id()));
    }

    frame_string.push_str(&format!("[{:?}]  ", frame.dlc()));

    for byte in frame.data() {
        frame_string.push_str(&format!("{:02X} ", byte));
    }

    frame_string
}
