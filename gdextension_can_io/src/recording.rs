use crate::{can_parser::CanParser, trace_filter::TraceFilter};
use crosscan::can::CanFrame;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
pub struct DecodedSignal {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Serialize)]
struct Record<'a> {
    timestamp_us: u64,
    interface: &'a str,
    id: u32,
    extended: bool,
    remote: bool,
    error: bool,
    dlc: usize,
    data: &'a [u8],
    message: String,
    signals: Vec<DecodedSignal>,
}

pub struct Recorder {
    raw: Option<BufWriter<File>>,
    decoded: Option<BufWriter<File>>,
    mf4: Option<Mf4Process>,
    parser: CanParser,
    decode: bool,
    pub paths: Vec<PathBuf>,
    last_flush: Instant,
}

impl Recorder {
    /// mode: 0 raw, 1 decoded, 2 both. File creation never overwrites a recording.
    pub fn start(
        directory: &Path,
        mode: i32,
        format: &str,
        parser: CanParser,
    ) -> Result<Self, String> {
        if !directory.is_dir() {
            return Err("Choose an existing recording directory.".into());
        }
        if !(0..=2).contains(&mode) || !["csv", "mf4"].contains(&format) {
            return Err("Invalid recording mode or format.".into());
        }
        if mode != 0 && parser.dbc().is_none() {
            return Err("Load a DBC before recording decoded signals.".into());
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let prefix = format!("cydervis-{stamp}-{}", std::process::id());
        let mut paths = Vec::new();
        let mut reserve = |suffix: &str| -> Result<(PathBuf, File), String> {
            let path = directory.join(format!("{prefix}-{suffix}.{format}"));
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
            paths.push(path.clone());
            Ok((path, file))
        };
        let mut raw = None;
        let mut decoded = None;
        let mut raw_path = None;
        let mut decoded_path = None;
        let result = (|| {
            if mode != 1 {
                let (path, file) = reserve("raw")?;
                raw_path = Some(path);
                if format == "csv" {
                    raw = Some(BufWriter::new(file));
                }
            }
            if mode != 0 {
                let (path, file) = reserve("decoded")?;
                decoded_path = Some(path);
                if format == "csv" {
                    decoded = Some(BufWriter::new(file));
                }
            }
            if let Some(file) = &mut raw {
                writeln!(
                    file,
                    "timestamp_us,interface,can_id,extended,remote,error,dlc,data"
                )
                .map_err(|e| e.to_string())?;
            }
            if let Some(file) = &mut decoded {
                writeln!(
                    file,
                    "timestamp_us,interface,can_id,extended,message,signal,value,unit"
                )
                .map_err(|e| e.to_string())?;
            }
            let mf4 = if format == "mf4" {
                Some(Mf4Process::start(
                    raw_path.as_deref(),
                    decoded_path.as_deref(),
                    &directory.join(format!("{prefix}-mf4.log")),
                )?)
            } else {
                None
            };
            Ok(mf4)
        })();
        match result {
            Ok(mf4) => {
                let mut recorder = Self {
                    raw,
                    decoded,
                    mf4,
                    parser,
                    decode: mode != 0,
                    paths,
                    last_flush: Instant::now(),
                };
                recorder.flush()?;
                Ok(recorder)
            }
            Err(error) => {
                drop(raw);
                drop(decoded);
                for path in paths {
                    let _ = std::fs::remove_file(path);
                }
                Err(error)
            }
        }
    }

    pub fn record(
        &mut self,
        frame: &CanFrame,
        interface: &str,
        filter: &TraceFilter,
    ) -> Result<(), String> {
        if !filter.matches(frame, interface) {
            return Ok(());
        }
        let (message, signals) = if self.decode {
            self.parser.decode_for_recording(frame).unwrap_or_default()
        } else {
            Default::default()
        };
        let record = Record {
            timestamp_us: frame.timestamp().unwrap_or_default(),
            interface,
            id: frame.id(),
            extended: frame.is_extended(),
            remote: frame.is_rtr(),
            error: frame.is_error(),
            dlc: frame.dlc(),
            data: frame.data(),
            message,
            signals,
        };
        let id = format!("{:X}", frame.id());
        let stamp = record.timestamp_us.to_string();
        let extended = frame.is_extended().to_string();
        if let Some(raw) = &mut self.raw {
            let payload = frame
                .data()
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            csv_row(
                raw,
                &[
                    &stamp,
                    interface,
                    &id,
                    &extended,
                    &frame.is_rtr().to_string(),
                    &frame.is_error().to_string(),
                    &frame.dlc().to_string(),
                    &payload,
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        if let Some(decoded) = &mut self.decoded {
            for signal in &record.signals {
                csv_row(
                    decoded,
                    &[
                        &stamp,
                        interface,
                        &id,
                        &extended,
                        &record.message,
                        &signal.name,
                        &signal.value.to_string(),
                        &signal.unit,
                    ],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        if let Some(mf4) = &mut self.mf4 {
            mf4.record(&record)?;
        }
        self.flush_if_due()
    }

    pub fn flush_if_due(&mut self) -> Result<(), String> {
        if self.last_flush.elapsed().as_millis() >= 250 {
            self.flush()?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), String> {
        if let Some(raw) = &mut self.raw {
            raw.flush().map_err(|e| e.to_string())?;
        }
        if let Some(decoded) = &mut self.decoded {
            decoded.flush().map_err(|e| e.to_string())?;
        }
        if let Some(mf4) = &mut self.mf4 {
            mf4.flush()?;
        }
        self.last_flush = Instant::now();
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), String> {
        self.flush()?;
        if let Some(mut mf4) = self.mf4.take() {
            mf4.finish()?;
        }
        Ok(())
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn csv_row(output: &mut impl Write, fields: &[&str]) -> std::io::Result<()> {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            write!(output, ",")?;
        }
        write!(output, "\"{}\"", field.replace('"', "\"\""))?;
    }
    writeln!(output)
}

struct Mf4Process {
    child: Child,
    input: Option<BufWriter<ChildStdin>>,
    log: PathBuf,
}

impl Mf4Process {
    fn start(raw: Option<&Path>, decoded: Option<&Path>, log: &Path) -> Result<Self, String> {
        let python = std::env::var_os("CYDERVIS_MF4_PYTHON").unwrap_or_else(|| "python".into());
        let mut command = Command::new(python);
        command
            .args(["-u", "-c", include_str!("mf4_writer.py")])
            .arg(raw.unwrap_or(Path::new("")))
            .arg(decoded.unwrap_or(Path::new("")))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(File::create(log).map_err(|e| e.to_string())?);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|e| format!("Cannot start Python for MF4: {e}. Install Python and python-can[mf4], or set CYDERVIS_MF4_PYTHON to your interpreter."))?;
        let mut ready = String::new();
        let result = BufReader::new(child.stdout.take().unwrap()).read_line(&mut ready);
        if result.is_err() || ready.trim() != "READY" {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "MF4 writer could not start. Install with: python -m pip install 'python-can[mf4]>=4.6,<5' 'asammdf>=8,<9'. Diagnostics: {}",
                std::fs::read_to_string(log).unwrap_or_default()
            ));
        }
        let input = child.stdin.take().map(BufWriter::new);
        Ok(Self {
            child,
            input,
            log: log.to_owned(),
        })
    }

    fn record(&mut self, frame: &Record) -> Result<(), String> {
        let input = self.input.as_mut().ok_or("MF4 recording has stopped")?;
        serde_json::to_writer(&mut *input, frame).map_err(|e| e.to_string())?;
        writeln!(input).map_err(|e| e.to_string())
    }

    fn flush(&mut self) -> Result<(), String> {
        if let Some(input) = &mut self.input {
            input.flush().map_err(|e| e.to_string())?;
        }
        if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!(
                "MF4 writer exited ({status}): {}",
                std::fs::read_to_string(&self.log).unwrap_or_default()
            ));
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), String> {
        let flush = self.input.take().map(|mut input| input.flush()).transpose();
        let status = self.child.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!(
                "MF4 finalization failed: {}",
                std::fs::read_to_string(&self.log).unwrap_or_default()
            ));
        }
        flush.map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl Drop for Mf4Process {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mf4_round_trip_when_python_configured() {
        let Some(python) = std::env::var_os("CYDERVIS_MF4_PYTHON") else {
            eprintln!("Set CYDERVIS_MF4_PYTHON to run the MF4 integration test");
            return;
        };
        let directory = std::env::temp_dir().join(format!("cyder mf4 {}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut parser = CanParser::new();
        parser
            .open_dbc("../godot/tests/transmit.dbc".into())
            .unwrap();
        let mut recorder = Recorder::start(&directory, 2, "mf4", parser).unwrap();
        let filter = TraceFilter::parse("0:0,#FFFFFFFF").unwrap();
        for stamp in 0..2050 {
            let mut frame =
                CanFrame::new_eff(0x10302, &[0x6f, 0x12, 0x83, 0x3a, 11, 1, 0, 0]).unwrap();
            frame.set_timestamp(Some(10000 + stamp * 1000));
            recorder.record(&frame, "can0", &filter).unwrap();
        }
        for mut frame in [
            CanFrame::new(0x123, &[0x12]).unwrap(),
            CanFrame::new_remote(0x124, 8, false).unwrap(),
            CanFrame::new_error(4).unwrap(),
        ] {
            frame.set_timestamp(Some(3_000_000));
            recorder.record(&frame, "can0", &filter).unwrap();
        }
        recorder.finish().unwrap();
        let output = Command::new(python)
            .arg("tests/verify_mf4.py")
            .args(&recorder.paths)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let paths = recorder.paths.clone();
        drop(recorder);
        for path in paths {
            std::fs::remove_file(path).unwrap();
        }
        for entry in std::fs::read_dir(&directory).unwrap() {
            std::fs::remove_file(entry.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn csv_records_all_filtered_frames_and_keeps_decoded_separate() {
        let directory =
            std::env::temp_dir().join(format!("cyder recording {}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut parser = CanParser::new();
        parser
            .open_dbc("../godot/tests/transmit.dbc".into())
            .unwrap();
        let mut recorder = Recorder::start(&directory, 2, "csv", parser).unwrap();
        let filter = TraceFilter::parse("00010302:DFFFFFFF").unwrap();
        for stamp in 0..250 {
            let mut frame =
                CanFrame::new_eff(0x10302, &[0x6f, 0x12, 0x83, 0x3a, 11, 1, 0, 0]).unwrap();
            frame.set_timestamp(Some(stamp));
            recorder.record(&frame, "can,\"0", &filter).unwrap();
            recorder
                .record(&CanFrame::new(0x123, &[0]).unwrap(), "can0", &filter)
                .unwrap();
        }
        recorder.finish().unwrap();
        let raw = std::fs::read_to_string(&recorder.paths[0]).unwrap();
        let decoded = std::fs::read_to_string(&recorder.paths[1]).unwrap();
        assert_eq!(raw.lines().count(), 251);
        assert_eq!(decoded.lines().count(), 751);
        assert!(raw.contains("\"can,\"\"0\""));
        assert!(raw.contains("6F 12 83 3A 0B 01 00 00"));
        assert!(decoded.contains("0.0010000000474974513"));
        let paths = recorder.paths.clone();
        drop(recorder);
        for path in paths {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn validates_recording_options_and_quotes_csv() {
        assert!(Recorder::start(Path::new("."), 1, "csv", CanParser::new()).is_err());
        assert!(Recorder::start(Path::new("."), 3, "csv", CanParser::new()).is_err());
        let mut output = Vec::new();
        csv_row(&mut output, &["comma,value", "quote\"value", "new\nline"]).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\"comma,value\",\"quote\"\"value\",\"new\nline\"\n"
        );
    }

    #[test]
    fn decoded_records_have_a_consistent_signal_set() {
        let mut parser = CanParser::new();
        parser
            .open_dbc("../godot/tests/transmit.dbc".into())
            .unwrap();
        let valid = CanFrame::new_eff(0x10302, &[0x6f, 0x12, 0x83, 0x3a, 11, 1, 0, 0]).unwrap();
        assert_eq!(parser.decode_for_recording(&valid).unwrap().1.len(), 3);
        let truncated = CanFrame::new_eff(0x10302, &[0x6f, 0x12, 0x83, 0x3a]).unwrap();
        assert!(parser.decode_for_recording(&truncated).is_none());
        let not_finite = CanFrame::new_eff(0x10302, &[0, 0, 0xc0, 0x7f, 11, 1, 0, 0]).unwrap();
        assert!(parser.decode_for_recording(&not_finite).is_none());
        assert!(
            parser
                .decode_for_recording(&CanFrame::new_remote(0x10302, 8, true).unwrap())
                .is_none()
        );
    }

    #[test]
    fn raw_decoded_and_empty_mf4_modes_finalize() {
        if std::env::var_os("CYDERVIS_MF4_PYTHON").is_none() {
            return;
        }
        let directory = std::env::temp_dir().join(format!("cyder modes {}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        for format in ["csv", "mf4"] {
            for mode in 0..=2 {
                let mut parser = CanParser::new();
                if mode != 0 {
                    parser
                        .open_dbc("../godot/tests/transmit.dbc".into())
                        .unwrap();
                }
                let mut recorder = Recorder::start(&directory, mode, format, parser).unwrap();
                assert_eq!(recorder.paths.len(), if mode == 2 { 2 } else { 1 });
                recorder.finish().unwrap();
                for path in &recorder.paths {
                    let bytes = std::fs::read(path).unwrap();
                    assert!(bytes.starts_with(if format == "mf4" {
                        b"MDF     "
                    } else {
                        b"timestamp_us"
                    }));
                }
            }
        }
        for entry in std::fs::read_dir(&directory).unwrap() {
            std::fs::remove_file(entry.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }
}
