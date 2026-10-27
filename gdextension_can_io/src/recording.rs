mod mf4;
pub(crate) mod reader;
mod viewer;

use crate::{can_parser::CanParser, trace_filter::TraceFilter};
use crosscan::can::CanFrame;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub struct DecodedSignal {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

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
    mf4: Option<mf4::Mf4Writer>,
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
        timeline_start: Option<Instant>,
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
        // Pair the file's UTC start with its position on the application's
        // receive timeline. MF4 stores offsets from this capture start.
        let timestamp_origin_us = timeline_start
            .map(|start| u64::try_from(start.elapsed().as_micros()))
            .transpose()
            .map_err(|_| "Recording time origin is out of range.")?
            .unwrap_or(0);
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
        let mut raw_mf4 = None;
        let mut decoded_mf4 = None;
        let result = (|| {
            if mode != 1 {
                let (_, file) = reserve("raw")?;
                if format == "csv" {
                    raw = Some(BufWriter::new(file));
                } else {
                    raw_mf4 = Some(file);
                }
            }
            if mode != 0 {
                let (_, file) = reserve("decoded")?;
                if format == "csv" {
                    decoded = Some(BufWriter::new(file));
                } else {
                    decoded_mf4 = Some(file);
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
                Some(mf4::Mf4Writer::start(
                    raw_mf4.take(),
                    decoded_mf4.take(),
                    u64::try_from(stamp).map_err(|_| "MF4 file start time is out of range.")?,
                    timestamp_origin_us,
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
                drop(raw_mf4);
                drop(decoded_mf4);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn native_mf4_round_trip() {
        let directory = std::env::temp_dir().join(format!("cyder mf4 {}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let dbc_path = directory.join("capture.dbc");
        let mut dbc = std::fs::read_to_string("../godot/tests/transmit.dbc").unwrap();
        dbc.push_str(concat!(
            "\nBO_ 291 Auxiliary: 3 Device\n",
            " SG_ voltage : 0|16@1+ (0.1,0) [0|6553.5] \"V\" PC\n",
            " SG_ temperature : 16|8@1- (0.5,-40) [-104|23.5] \"°C\" PC\n",
        ));
        std::fs::write(&dbc_path, dbc).unwrap();
        let mut parser = CanParser::new();
        parser.open_dbc(dbc_path.to_str().unwrap().into()).unwrap();
        let mut recorder = Recorder::start(&directory, 2, "mf4", parser, None).unwrap();
        let filter = TraceFilter::parse("0:0,#FFFFFFFF").unwrap();
        for stamp in 0..6000 {
            let mut frame =
                CanFrame::new_eff(0x10302, &[0x6f, 0x12, 0x83, 0x3a, 11, 1, 0, 0]).unwrap();
            frame.set_timestamp(Some(10000 + stamp * 1000));
            recorder.record(&frame, "can0", &filter).unwrap();
            if stamp % 1000 == 0 {
                let mut auxiliary = CanFrame::new(0x123, &[123, 0, 246]).unwrap();
                auxiliary.set_timestamp(Some(10500 + stamp * 1000));
                recorder.record(&auxiliary, "sensor °C", &filter).unwrap();
            }
        }
        recorder.flush().unwrap();
        for (index, mut frame) in [
            CanFrame::new(0x123, &[0x12]).unwrap(),
            CanFrame::new(0x123, &[]).unwrap(),
            CanFrame::new_remote(0x124, 8, false).unwrap(),
            CanFrame::new_remote(0x1ABCDE, 3, true).unwrap(),
            CanFrame::new_error(4).unwrap(),
            CanFrame::new_eff(0x10302, &[0x6f, 0x12, 0x83, 0x3a]).unwrap(),
            CanFrame::new_eff(0x10302, &[0, 0, 0xc0, 0x7f, 11, 1, 0, 0]).unwrap(),
        ]
        .into_iter()
        .enumerate()
        {
            frame.set_timestamp(Some(7_000_000 + index as u64 * 1000));
            recorder.record(&frame, "can0", &filter).unwrap();
        }
        recorder
            .record(
                &CanFrame::new(0x456, &[0xFF]).unwrap(),
                "can0",
                &TraceFilter::parse("123:7FF").unwrap(),
            )
            .unwrap();
        recorder.finish().unwrap();
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let mut dbc_decoder = CanParser::new();
        dbc_decoder
            .open_dbc(dbc_path.to_str().unwrap().into())
            .unwrap();
        let mut raw_capture = reader::Capture::open(&recorder.paths[0], &cancelled).unwrap();
        assert_eq!(
            raw_capture
                .info()
                .groups
                .iter()
                .map(|g| g.records)
                .sum::<u64>(),
            6013
        );
        let page = raw_capture.page(0, 0, 1, &dbc_decoder, &cancelled).unwrap();
        assert_eq!(page.rows[0][2], "10302");
        assert_eq!(page.rows[0][5], "6F 12 83 3A 0B 01 00 00");
        assert!(page.rows[0][6].contains("SetComplementaryFilter.odr = 11"));
        assert_eq!(
            raw_capture
                .page(1, 5, 500, &dbc_decoder, &cancelled)
                .unwrap()
                .rows[0][1],
            "sensor °C / bus 1"
        );
        let mut decoded_capture = reader::Capture::open(&recorder.paths[1], &cancelled).unwrap();
        assert_eq!(decoded_capture.info().groups[0].records, 6000);
        let page = decoded_capture
            .page(0, 5999, 500, &dbc_decoder, &cancelled)
            .unwrap();
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0][2], "11");
        assert!(
            decoded_capture.info().groups[1]
                .columns
                .contains(&"Auxiliary.temperature (°C)".into())
        );
        drop(raw_capture);
        drop(decoded_capture);
        if let Some(python) = std::env::var_os("CYDERVIS_MF4_VERIFY_PYTHON") {
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
        }
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
        let mut recorder = Recorder::start(&directory, 2, "csv", parser, None).unwrap();
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
        assert!(Recorder::start(Path::new("."), 1, "csv", CanParser::new(), None).is_err());
        assert!(Recorder::start(Path::new("."), 3, "csv", CanParser::new(), None).is_err());
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
                let mut recorder = Recorder::start(&directory, mode, format, parser, None).unwrap();
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
