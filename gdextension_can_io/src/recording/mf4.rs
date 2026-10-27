//! Native MDF 4.10 writer for classic CAN bus events and decoded float64 signals.
//!
//! Each group has a fixed record layout and a bounded buffer. DT chunks are
//! linked through variable-offset DL blocks, allowing groups to be interleaved
//! without retaining a capture or its complete block index in memory.
//! Layout references: ASAM MDF 4.1 and python-can's CAN bus logging channel names.
use super::Record;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};

const CHUNK_BYTES: usize = 64 * 1024;
const MAX_GROUPS: usize = 4096;
const HEADER: u64 = 64;
const BUS_EVENT: u16 = 2;
const BUS_CHANNEL: u32 = 1 << 10;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum FrameKind {
    Data,
    Remote,
    Error,
}

impl FrameKind {
    fn name(self) -> &'static str {
        match self {
            Self::Data => "CAN_DataFrame",
            Self::Remote => "CAN_RemoteFrame",
            Self::Error => "CAN_ErrorFrame",
        }
    }
    fn record_size(self) -> usize {
        if self == Self::Remote { 17 } else { 25 }
    }
}

pub(super) struct Mf4Writer {
    raw: Option<MdfFile<BufWriter<File>>>,
    decoded: Option<MdfFile<BufWriter<File>>>,
    raw_groups: HashMap<(String, FrameKind), usize>,
    decoded_groups: HashMap<(String, u32, bool, String), usize>,
    buses: HashMap<String, u8>,
    finished: bool,
    failure: Option<String>,
    timestamp_offset_us: i128,
}

impl Mf4Writer {
    pub(super) fn start(
        raw: Option<File>,
        decoded: Option<File>,
        epoch_ns: u64,
        timestamp_offset_us: i128,
    ) -> Result<Self, String> {
        Ok(Self {
            raw: raw
                .map(|file| MdfFile::new(BufWriter::new(file), epoch_ns))
                .transpose()
                .map_err(|e| e.to_string())?,
            decoded: decoded
                .map(|file| MdfFile::new(BufWriter::new(file), epoch_ns))
                .transpose()
                .map_err(|e| e.to_string())?,
            raw_groups: HashMap::new(),
            decoded_groups: HashMap::new(),
            buses: HashMap::new(),
            finished: false,
            failure: None,
            timestamp_offset_us,
        })
    }

    pub(super) fn record(&mut self, record: &Record<'_>) -> Result<(), String> {
        self.check_open()?;
        let result = self.write_record(record);
        self.track(result)
    }

    fn write_record(&mut self, record: &Record<'_>) -> io::Result<()> {
        // Apply the selected origin in integer microseconds before conversion.
        // Negative offsets are valid for queued frames captured before Start.
        let timestamp_s = (record.timestamp_us as i128 + self.timestamp_offset_us) as f64 * 1e-6;
        if record.dlc > 8 || record.data.len() > 8 {
            return Err(invalid(
                "MF4 capture supports classic CAN payloads up to 8 bytes.",
            ));
        }
        if let Some(raw) = &mut self.raw {
            let bus = if let Some(&bus) = self.buses.get(record.interface) {
                bus
            } else {
                let bus = u8::try_from(self.buses.len())
                    .map_err(|_| invalid("Too many CAN interfaces in one capture."))?;
                self.buses.insert(record.interface.into(), bus);
                bus
            };
            let kind = if record.remote {
                FrameKind::Remote
            } else if record.error {
                FrameKind::Error
            } else {
                FrameKind::Data
            };
            let key = (record.interface.to_owned(), kind);
            let group = if let Some(&group) = self.raw_groups.get(&key) {
                group
            } else {
                let group = raw.can_group(record.interface, kind)?;
                self.raw_groups.insert(key, group);
                group
            };
            let mut bytes = [0u8; 25];
            bytes[..8].copy_from_slice(&timestamp_s.to_le_bytes());
            bytes[8] = bus;
            bytes[9..13].copy_from_slice(&record.id.to_le_bytes());
            bytes[13] = u8::from(record.extended);
            bytes[14] = record.dlc as u8;
            if kind != FrameKind::Remote {
                bytes[15] = record.data.len() as u8;
                bytes[16..16 + record.data.len()].copy_from_slice(record.data);
            }
            // Dir=0 (received). A remote frame has a requested DLC but no data.
            raw.record(group, &bytes[..kind.record_size()])?;
        }
        if let Some(decoded) = &mut self.decoded {
            if record.signals.is_empty() {
                return Ok(());
            }
            if record.signals.len() >= CHUNK_BYTES / 8 {
                return Err(invalid("MF4 capture has too many signals in one message."));
            }
            let key = (
                record.interface.to_owned(),
                record.id,
                record.extended,
                record.message.clone(),
            );
            let group = if let Some(&group) = self.decoded_groups.get(&key) {
                group
            } else {
                let specs = record
                    .signals
                    .iter()
                    .enumerate()
                    .map(|(index, signal)| Channel {
                        name: format!("{}.{}", record.message, signal.name),
                        unit: signal.unit.clone(),
                        offset: ((index + 1) * 8) as u32,
                        bits: 64,
                        data_type: 4,
                        master: false,
                        flags: 0,
                    })
                    .collect::<Vec<_>>();
                let name = format!(
                    "{} {:X} {} {}",
                    record.interface,
                    record.id,
                    if record.extended {
                        "extended"
                    } else {
                        "standard"
                    },
                    record.message
                );
                let group = decoded.signal_group(&name, record.interface, &specs)?;
                self.decoded_groups.insert(key, group);
                group
            };
            let mut bytes = Vec::with_capacity((record.signals.len() + 1) * 8);
            bytes.extend_from_slice(&timestamp_s.to_le_bytes());
            for signal in &record.signals {
                bytes.extend_from_slice(&signal.value.to_le_bytes());
            }
            decoded.record(group, &bytes)?;
        }
        Ok(())
    }

    pub(super) fn flush(&mut self) -> Result<(), String> {
        self.check_open()?;
        let result = self.flush_files();
        self.track(result)
    }

    fn flush_files(&mut self) -> io::Result<()> {
        if let Some(raw) = &mut self.raw {
            raw.flush()?;
        }
        if let Some(decoded) = &mut self.decoded {
            decoded.flush()?;
        }
        Ok(())
    }

    pub(super) fn finish(&mut self) -> Result<(), String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        let result = self.flush_files();
        self.track(result)
    }

    fn check_open(&self) -> Result<(), String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.finished {
            return Err("MF4 recording has stopped.".into());
        }
        Ok(())
    }

    fn track(&mut self, result: io::Result<()>) -> Result<(), String> {
        result.map_err(|error| {
            let error = format!("MF4 write failed: {error}");
            self.failure = Some(error.clone());
            error
        })
    }
}

impl Drop for Mf4Writer {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

struct Channel {
    name: String,
    unit: String,
    offset: u32,
    bits: u32,
    data_type: u8,
    master: bool,
    flags: u32,
}

impl Channel {
    fn time() -> Self {
        Self {
            name: "time".into(),
            unit: "s".into(),
            offset: 0,
            bits: 64,
            data_type: 4,
            master: true,
            flags: 0,
        }
    }
    fn bus(name: String, offset: u32, bits: u32, data_type: u8) -> Self {
        Self {
            name,
            unit: String::new(),
            offset,
            bits,
            data_type,
            master: false,
            flags: BUS_CHANNEL,
        }
    }
}

struct Group {
    cg: u64,
    dg: u64,
    last_dl: u64,
    stream_bytes: u64,
    cycles: u64,
    record_size: usize,
    pending: Vec<u8>,
}

struct MdfFile<W: Write + Seek> {
    output: W,
    end: u64,
    last_dg: u64,
    groups: Vec<Group>,
}

impl<W: Write + Seek> MdfFile<W> {
    fn new(mut output: W, epoch_ns: u64) -> io::Result<Self> {
        let mut id = [0u8; 64];
        id[..8].copy_from_slice(b"MDF     ");
        id[8..16].copy_from_slice(b"4.10    ");
        id[16..24].copy_from_slice(b"CyderVis");
        id[28..30].copy_from_slice(&410u16.to_le_bytes());
        output.write_all(&id)?;
        let mut file = Self {
            output,
            end: 64,
            last_dg: 0,
            groups: Vec::new(),
        };
        let mut data = [0u8; 32];
        data[..8].copy_from_slice(&epoch_ns.to_le_bytes());
        data[12] = 2; // UTC offsets are valid and zero.
        file.block(b"##HD", &[0; 6], &data)?;
        file.output.flush()?;
        Ok(file)
    }

    fn block(&mut self, id: &[u8; 4], links: &[u64], data: &[u8]) -> io::Result<u64> {
        let padding = (8 - self.end % 8) % 8;
        self.output.write_all(&[0u8; 8][..padding as usize])?;
        self.end += padding;
        let position = self.end;
        let size = 24 + links.len() as u64 * 8 + data.len() as u64;
        self.output.write_all(id)?;
        self.output.write_all(&[0; 4])?;
        self.output.write_all(&size.to_le_bytes())?;
        self.output.write_all(&(links.len() as u64).to_le_bytes())?;
        for link in links {
            self.output.write_all(&link.to_le_bytes())?;
        }
        self.output.write_all(data)?;
        self.end += size;
        Ok(position)
    }

    fn patch(&mut self, offset: u64, value: u64) -> io::Result<()> {
        self.output.seek(SeekFrom::Start(offset))?;
        self.output.write_all(&value.to_le_bytes())?;
        self.output.seek(SeekFrom::Start(self.end))?;
        Ok(())
    }

    fn text(&mut self, text: &str) -> io::Result<u64> {
        if text.is_empty() {
            return Ok(0);
        }
        if text.contains('\0') {
            return Err(invalid(
                "MDF names and units cannot contain NUL characters.",
            ));
        }
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        bytes.resize(bytes.len().div_ceil(8) * 8, 0);
        self.block(b"##TX", &[], &bytes)
    }

    fn source(&mut self, interface: &str) -> io::Result<u64> {
        let name = self.text(interface)?;
        self.block(b"##SI", &[name, name, 0], &[2, 2, 0, 0, 0, 0, 0, 0])
    }

    fn channel(&mut self, channel: &Channel, component: u64, source: u64) -> io::Result<u64> {
        let name = self.text(&channel.name)?;
        let unit = self.text(&channel.unit)?;
        let mut data = [0u8; 72];
        data[0] = if channel.master { 2 } else { 0 };
        data[1] = u8::from(channel.master);
        data[2] = channel.data_type;
        data[4..8].copy_from_slice(&channel.offset.to_le_bytes());
        data[8..12].copy_from_slice(&channel.bits.to_le_bytes());
        data[12..16].copy_from_slice(&channel.flags.to_le_bytes());
        self.block(b"##CN", &[0, component, name, source, 0, 0, unit, 0], &data)
    }

    fn link_channels(&mut self, channels: &[Channel]) -> io::Result<u64> {
        let mut first = 0;
        let mut previous = 0;
        for channel in channels {
            let position = self.channel(channel, 0, 0)?;
            if previous == 0 {
                first = position;
            } else {
                self.patch(previous + 24, position)?;
            }
            previous = position;
        }
        Ok(first)
    }

    fn group(
        &mut self,
        name: &str,
        interface: &str,
        first_channel: u64,
        size: usize,
        flags: u16,
    ) -> io::Result<usize> {
        if self.groups.len() >= MAX_GROUPS || size > CHUNK_BYTES {
            return Err(invalid("MF4 capture has too many groups or signals."));
        }
        let name = self.text(name)?;
        let source = self.source(interface)?;
        let mut data = [0u8; 32];
        data[16..18].copy_from_slice(&flags.to_le_bytes());
        data[24..28].copy_from_slice(&(size as u32).to_le_bytes());
        let cg = self.block(b"##CG", &[0, first_channel, name, source, 0, 0], &data)?;
        let dg = self.block(b"##DG", &[0, cg, 0, 0], &[0; 8])?;
        self.patch(
            if self.last_dg == 0 {
                HEADER + 24
            } else {
                self.last_dg + 24
            },
            dg,
        )?;
        self.last_dg = dg;
        self.groups.push(Group {
            cg,
            dg,
            last_dl: 0,
            stream_bytes: 0,
            cycles: 0,
            record_size: size,
            pending: Vec::new(),
        });
        Ok(self.groups.len() - 1)
    }

    fn can_group(&mut self, interface: &str, kind: FrameKind) -> io::Result<usize> {
        let name = kind.name();
        let mut fields = vec![
            Channel::bus(format!("{name}.BusChannel"), 8, 8, 0),
            Channel::bus(format!("{name}.ID"), 9, 32, 0),
            Channel::bus(format!("{name}.IDE"), 13, 8, 0),
            Channel::bus(format!("{name}.DLC"), 14, 8, 0),
            Channel::bus(format!("{name}.DataLength"), 15, 8, 0),
        ];
        if kind != FrameKind::Remote {
            fields.push(Channel::bus(format!("{name}.DataBytes"), 16, 64, 10));
        }
        fields.push(Channel::bus(
            format!("{name}.Dir"),
            (kind.record_size() - 1) as u32,
            8,
            0,
        ));
        let components = self.link_channels(&fields)?;
        let source = self.source(interface)?;
        let composite = self.channel(
            &Channel::bus(name.into(), 8, ((kind.record_size() - 8) * 8) as u32, 10),
            components,
            source,
        )?;
        let time = self.channel(&Channel::time(), 0, 0)?;
        self.patch(time + 24, composite)?;
        self.group(
            &format!("{interface} {name}"),
            interface,
            time,
            kind.record_size(),
            BUS_EVENT,
        )
    }

    fn signal_group(
        &mut self,
        name: &str,
        interface: &str,
        signals: &[Channel],
    ) -> io::Result<usize> {
        let first = self.link_channels(signals)?;
        let time = self.channel(&Channel::time(), 0, 0)?;
        self.patch(time + 24, first)?;
        self.group(name, interface, time, (signals.len() + 1) * 8, 0)
    }

    fn record(&mut self, group: usize, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() != self.groups[group].record_size {
            return Err(invalid("MF4 record does not match its channel layout."));
        }
        if self.groups[group].pending.len() + bytes.len() > CHUNK_BYTES {
            self.flush_group(group)?;
        }
        self.groups[group].pending.extend_from_slice(bytes);
        Ok(())
    }

    fn flush_group(&mut self, index: usize) -> io::Result<()> {
        if self.groups[index].pending.is_empty() {
            return Ok(());
        }
        let bytes = std::mem::take(&mut self.groups[index].pending);
        let dt = self.block(b"##DT", &[], &bytes)?;
        let mut data = [0u8; 16];
        data[4..8].copy_from_slice(&1u32.to_le_bytes()); // One variable-offset DT per list node.
        data[8..16].copy_from_slice(&self.groups[index].stream_bytes.to_le_bytes());
        let dl = self.block(b"##DL", &[0, dt], &data)?;
        let group = &self.groups[index];
        let link = if group.last_dl == 0 {
            group.dg + 40
        } else {
            group.last_dl + 24
        };
        let cycles = group
            .cycles
            .checked_add((bytes.len() / group.record_size) as u64)
            .ok_or_else(|| invalid("MF4 sample count overflow."))?;
        let stream_bytes = group
            .stream_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid("MF4 data size overflow."))?;
        let cg = group.cg;
        self.patch(link, dl)?;
        self.patch(cg + 80, cycles)?;
        let group = &mut self.groups[index];
        group.last_dl = dl;
        group.cycles = cycles;
        group.stream_bytes = stream_bytes;
        group.pending = bytes;
        group.pending.clear();
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        for index in 0..self.groups.len() {
            self.flush_group(index)?;
        }
        self.output.flush()
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn u64_at(bytes: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
    }

    #[test]
    fn chunks_interleave_without_growing_an_index_or_overlapping_data() {
        let mut file = MdfFile::new(Cursor::new(Vec::new()), 123_456).unwrap();
        let a = file.can_group("bench", FrameKind::Data).unwrap();
        let b = file.can_group("bench", FrameKind::Remote).unwrap();
        for index in 0..10000 {
            file.record(a, &[0xAA; 25]).unwrap();
            if index % 3000 == 0 {
                file.record(b, &[0xBB; 17]).unwrap();
                file.flush().unwrap();
            }
            assert!(file.groups[a].pending.len() <= CHUNK_BYTES);
        }
        file.flush().unwrap();
        assert_eq!(file.groups[a].cycles, 10000);
        assert_eq!(file.groups[b].cycles, 4);
        let bytes = file.output.get_ref();
        for (group, pattern, expected) in [(a, 0xAA, 250000), (b, 0xBB, 68)] {
            let mut dl = u64_at(bytes, file.groups[group].dg as usize + 40) as usize;
            let mut size = 0;
            while dl != 0 {
                assert_eq!(dl % 8, 0);
                assert_eq!(&bytes[dl..dl + 4], b"##DL");
                assert_eq!(u64_at(bytes, dl + 48), size);
                let dt = u64_at(bytes, dl + 32) as usize;
                assert_eq!(dt % 8, 0);
                assert_eq!(&bytes[dt..dt + 4], b"##DT");
                let count = u64_at(bytes, dt + 8) as usize - 24;
                assert!(count <= CHUNK_BYTES);
                assert!(
                    bytes[dt + 24..dt + 24 + count]
                        .iter()
                        .all(|&byte| byte == pattern)
                );
                size += count as u64;
                dl = u64_at(bytes, dl + 24) as usize;
            }
            assert_eq!(size, expected);
        }
        assert_eq!(u64_at(bytes, HEADER as usize + 72), 123_456);
    }

    #[test]
    fn rejects_inconsistent_records_and_invalid_text() {
        let mut file = MdfFile::new(Cursor::new(Vec::new()), 0).unwrap();
        let group = file.can_group("bench", FrameKind::Data).unwrap();
        assert!(file.record(group, &[0; 24]).is_err());
        assert!(file.text("bad\0name").is_err());
        file.flush().unwrap();
        assert_eq!(file.groups[group].cycles, 0);
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Fault {
        Write,
        Seek,
        Flush,
    }

    struct FaultyOutput {
        bytes: Cursor<Vec<u8>>,
        fault: Option<Fault>,
    }

    impl Write for FaultyOutput {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fault == Some(Fault::Write) {
                Err(io::Error::other("injected write failure"))
            } else {
                self.bytes.write(bytes)
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.fault == Some(Fault::Flush) {
                Err(io::Error::other("injected flush failure"))
            } else {
                Ok(())
            }
        }
    }

    impl Seek for FaultyOutput {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            if self.fault == Some(Fault::Seek) {
                Err(io::Error::other("injected seek failure"))
            } else {
                self.bytes.seek(position)
            }
        }
    }

    #[test]
    fn capture_time_origin_preserves_hardware_intervals_in_both_files() {
        use crate::can_parser::CanParser;
        use crate::recording::{DecodedSignal, reader::Capture};
        use std::sync::atomic::AtomicBool;

        let directory = std::env::temp_dir().join(format!("cyder clock {}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let raw = directory.join("raw.mf4");
        let decoded = directory.join("decoded.mf4");
        let origin = 3_600_000_000u64; // Start recording an hour after app launch.
        let mut writer = Mf4Writer::start(
            Some(File::create(&raw).unwrap()),
            Some(File::create(&decoded).unwrap()),
            1_710_000_000_000_000_000,
            -i128::from(origin),
        )
        .unwrap();
        for timestamp_us in [origin - 1000, origin + 5000, origin + 6000] {
            writer
                .record(&Record {
                    timestamp_us,
                    interface: "bench",
                    id: 0x123,
                    extended: false,
                    remote: false,
                    error: false,
                    dlc: 1,
                    data: &[42],
                    message: "Sensor".into(),
                    signals: vec![DecodedSignal {
                        name: "value".into(),
                        value: 42.,
                        unit: "V".into(),
                    }],
                })
                .unwrap();
        }
        writer.finish().unwrap();
        drop(writer);
        for path in [&raw, &decoded] {
            let cancelled = AtomicBool::new(false);
            let mut capture = Capture::open(path, &cancelled).unwrap();
            assert_eq!(capture.info().start_time_ns, "1710000000000000000");
            let page = capture
                .page(0, 0, 500, &CanParser::new(), &cancelled)
                .unwrap();
            assert_eq!(
                page.rows
                    .iter()
                    .map(|row| row[0].as_str())
                    .collect::<Vec<_>>(),
                ["-0.001", "0.005", "0.006"]
            );
        }
        std::fs::remove_file(raw).unwrap();
        std::fs::remove_file(decoded).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn checkpoint_reports_write_seek_and_flush_failures() {
        for fault in [Fault::Write, Fault::Seek, Fault::Flush] {
            let output = FaultyOutput {
                bytes: Cursor::new(Vec::new()),
                fault: None,
            };
            let mut file = MdfFile::new(output, 0).unwrap();
            let group = file.can_group("bench", FrameKind::Data).unwrap();
            file.record(group, &[0; 25]).unwrap();
            file.output.fault = Some(fault);
            assert!(file.flush().unwrap_err().to_string().contains("injected"));
        }
    }
}
