//! Paged, read-only MDF 4 reader. Data stays on disk; only a requested page and
//! at most one bounded deflate block are decoded. See ASAM MDF 4.1 block layouts.
use crate::can_parser::CanParser;
use crosscan::can::CanFrame;
use flate2::read::ZlibDecoder;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

type Result<T> = std::result::Result<T, String>;
const MAX_BLOCKS: usize = 1_000_000;
const MAX_GROUPS: usize = 4096;
const MAX_CHANNELS: usize = 256;
const MAX_METADATA: u64 = 1024 * 1024;
const METADATA_BUDGET: u64 = 64 * 1024 * 1024;
const MAX_RECORD: usize = 65536;
const MAX_INFLATED: u64 = 128 * 1024 * 1024;
const MAX_INDEX: usize = 16_000_000;

#[derive(Clone, Serialize)]
pub struct GroupInfo {
    pub name: String,
    pub kind: &'static str,
    pub description: &'static str,
    pub records: u64,
    pub columns: Vec<String>,
}

#[derive(Serialize)]
pub struct CaptureInfo {
    pub version: String,
    pub start_time_ns: String,
    pub groups: Vec<GroupInfo>,
}

#[derive(Serialize)]
pub struct Page {
    pub group: usize,
    pub offset: u64,
    pub total: u64,
    pub rows: Vec<Vec<String>>,
}

struct Block {
    id: [u8; 4],
    links: Vec<u64>,
    data: u64,
    size: u64,
}

#[derive(Clone)]
struct Segment {
    start: u64,
    data: u64,
    size: u64,
    stored: u64,
    compression: Option<(u8, u32)>,
}

struct Stream {
    segments: Vec<Segment>,
    size: u64,
}

enum Conversion {
    Identity,
    Linear(f64, f64),
    Rational([f64; 6]),
    Unsupported(u8),
}

struct Channel {
    name: String,
    unit: String,
    channel_type: u8,
    sync_type: u8,
    data_type: u8,
    offset: usize,
    shift: u8,
    bits: u32,
    flags: u32,
    invalid_bit: u32,
    conversion: Conversion,
}

enum Value {
    Unsigned(u64),
    Signed(i64),
    Float(f64),
    Bytes(Vec<u8>),
    Text(String),
    Invalid,
}

impl Value {
    fn number(&self) -> Option<f64> {
        match self {
            Self::Unsigned(v) => Some(*v as f64),
            Self::Signed(v) => Some(*v as f64),
            Self::Float(v) => Some(*v),
            _ => None,
        }
    }
    fn unsigned(&self) -> Result<u64> {
        match self {
            Self::Unsigned(v) => Ok(*v),
            Self::Signed(v) if *v >= 0 => Ok(*v as u64),
            Self::Float(v) if v.is_finite() && *v >= 0.0 && v.fract() == 0.0 => Ok(*v as u64),
            _ => Err("Invalid CAN event field.".into()),
        }
    }
    fn display(&self) -> String {
        match self {
            Self::Unsigned(v) => v.to_string(),
            Self::Signed(v) => v.to_string(),
            Self::Float(v) => v.to_string(),
            Self::Bytes(bytes) => hex(bytes),
            Self::Text(text) => text.clone(),
            Self::Invalid => "N/A".into(),
        }
    }
}

struct Group {
    info: GroupInfo,
    stream: usize,
    record_id: u64,
    record_size: usize,
    data_size: usize,
    channels: Vec<Channel>,
    master: usize,
    interface: String,
    event: Option<&'static str>,
    // Sorted groups address records directly. Unsorted groups index record
    // positions once, rather than loading all payloads or rescanning each page.
    positions: Option<Vec<u64>>,
}

pub struct Capture {
    file: File,
    length: u64,
    info: CaptureInfo,
    groups: Vec<Group>,
    streams: Vec<Stream>,
    cache: Option<(u64, Vec<u8>)>,
    metadata_remaining: u64,
    index_remaining: usize,
    blocks_remaining: usize,
}

impl Capture {
    pub fn open(path: &Path, cancelled: &AtomicBool) -> Result<Self> {
        let file = File::open(path).map_err(|e| format!("Cannot open MF4: {e}"))?;
        let length = file.metadata().map_err(io_error)?.len();
        let mut capture = Self {
            file,
            length,
            info: CaptureInfo {
                version: String::new(),
                start_time_ns: String::new(),
                groups: Vec::new(),
            },
            groups: Vec::new(),
            streams: Vec::new(),
            cache: None,
            metadata_remaining: METADATA_BUDGET,
            index_remaining: MAX_INDEX,
            blocks_remaining: MAX_BLOCKS,
        };
        let id = capture.bytes(0, 64)?;
        if &id[..8] != b"MDF     " {
            return Err("Choose a finalized MDF 4 (.mf4) file.".into());
        }
        let version = u16::from_le_bytes(id[28..30].try_into().unwrap());
        if !(400..500).contains(&version) {
            return Err(format!(
                "MDF version {version} is unsupported; MDF 4 is required."
            ));
        }
        capture.info.version = String::from_utf8_lossy(&id[8..16]).trim().into();
        let header = capture.expected(64, b"##HD", 6, 32)?;
        capture.info.start_time_ns = u64_at(&capture.payload(&header)?, 0)?.to_string();
        let mut dg = header.links[0];
        let mut seen = HashSet::new();
        while dg != 0 {
            check_cancelled(cancelled)?;
            visit(&mut seen, dg)?;
            let block = capture.expected(dg, b"##DG", 4, 8)?;
            let record_id_size = capture.payload(&block)?[0] as usize;
            if ![0, 1, 2, 4, 8].contains(&record_id_size) {
                return Err("Invalid MDF record ID size.".into());
            }
            let mut stream = Stream {
                segments: Vec::new(),
                size: 0,
            };
            capture.data_blocks(block.links[2], &mut stream, cancelled)?;
            let stream_index = capture.streams.len();
            capture.streams.push(stream);
            let first_group = capture.groups.len();
            let mut cg = block.links[1];
            let mut cg_seen = HashSet::new();
            while cg != 0 {
                check_cancelled(cancelled)?;
                visit(&mut cg_seen, cg)?;
                let block = capture.expected(cg, b"##CG", 6, 32)?;
                let data = capture.payload(&block)?;
                if u16_at(&data, 16)? & 1 != 0 {
                    return Err("Variable-length MDF record groups are not supported.".into());
                }
                let data_size = u32_at(&data, 24)? as usize;
                let invalid_size = u32_at(&data, 28)? as usize;
                let size = data_size
                    .checked_add(invalid_size)
                    .filter(|&s| s > 0 && s <= MAX_RECORD)
                    .ok_or("MDF records must be between 1 and 65,536 bytes.")?;
                let mut channels = Vec::new();
                capture.channels(block.links[1], &mut channels, &mut HashSet::new(), 0)?;
                let master = channels
                    .iter()
                    .position(|c| [2, 3].contains(&c.channel_type) && c.sync_type == 1)
                    .ok_or("A channel group has no supported time master.")?;
                for channel in &channels {
                    if ![3, 6].contains(&channel.channel_type)
                        && (channel.offset as u64 * 8 + channel.shift as u64 + channel.bits as u64
                            > data_size as u64 * 8)
                    {
                        return Err(format!(
                            "Channel {} extends beyond its record.",
                            channel.name
                        ));
                    }
                    if channel.flags & 2 != 0 && channel.invalid_bit as usize >= invalid_size * 8 {
                        return Err("MDF invalidation bit extends beyond its record.".into());
                    }
                }
                let event = [
                    "CAN_DataFrame",
                    "CAN_RemoteFrame",
                    "CAN_ErrorFrame",
                    "CANFD_DataFrame",
                ]
                .into_iter()
                .find(|name| channels.iter().any(|c| c.name == format!("{name}.ID")));
                let name = capture.text(block.links[2])?;
                let mut interface = String::new();
                if block.links[3] != 0 {
                    let source = capture.expected(block.links[3], b"##SI", 3, 8)?;
                    interface = capture.text(source.links[0])?;
                    if interface.is_empty() {
                        interface = capture.text(source.links[1])?;
                    }
                }
                let time_column = format!(
                    "Time ({})",
                    if channels[master].unit.is_empty() {
                        "s"
                    } else {
                        &channels[master].unit
                    }
                );
                let mut columns = if event.is_some() {
                    [
                        "Time (s)",
                        "Interface",
                        "CAN ID (hex)",
                        "Flags",
                        "DLC",
                        "Data (hex)",
                        "Decoded signals",
                    ]
                    .map(str::to_owned)
                    .to_vec()
                } else {
                    std::iter::once("Time (s)".into())
                        .chain(
                            channels
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| *i != master)
                                .map(|(_, c)| {
                                    if c.unit.is_empty() {
                                        c.name.clone()
                                    } else {
                                        format!("{} ({})", c.name, c.unit)
                                    }
                                }),
                        )
                        .collect()
                };
                columns[0] = time_column;
                let info = GroupInfo {
                    name: if name.is_empty() {
                        event
                            .map(|event| format!("{interface} {event}").trim().to_owned())
                            .unwrap_or_else(|| format!("Group {}", capture.groups.len() + 1))
                    } else {
                        name
                    },
                    kind: if event.is_some() { "can" } else { "signals" },
                    description: match event {
                        Some("CAN_RemoteFrame") => "Remote CAN frames",
                        Some("CAN_ErrorFrame") => "CAN error frames",
                        Some("CANFD_DataFrame") => "CAN FD frames",
                        Some(_) => "CAN data frames",
                        None => "Signals",
                    },
                    records: u64_at(&data, 8)?,
                    columns,
                };
                capture.groups.push(Group {
                    info,
                    stream: stream_index,
                    record_id: u64_at(&data, 0)?,
                    record_size: size,
                    data_size,
                    channels,
                    master,
                    interface,
                    event,
                    positions: None,
                });
                if capture.groups.len() > MAX_GROUPS {
                    return Err("MF4 has too many channel groups.".into());
                }
                cg = block.links[0];
            }
            capture.index_stream(stream_index, first_group, record_id_size, cancelled)?;
            dg = block.links[0];
        }
        capture.info.groups = capture.groups.iter().map(|g| g.info.clone()).collect();
        Ok(capture)
    }

    pub fn info(&self) -> &CaptureInfo {
        &self.info
    }

    pub fn page(
        &mut self,
        group: usize,
        offset: u64,
        limit: usize,
        parser: &CanParser,
        cancelled: &AtomicBool,
    ) -> Result<Page> {
        let target = self.groups.get(group).ok_or("Unknown MDF channel group.")?;
        let total = target.info.records;
        let offset = offset.min(total);
        let count = (total - offset).min(limit.min(500) as u64);
        let mut rows = Vec::new();
        for index in offset..offset + count {
            check_cancelled(cancelled)?;
            let target = &self.groups[group];
            let position = if let Some(positions) = &target.positions {
                positions[index as usize]
            } else {
                index
                    .checked_mul(target.record_size as u64)
                    .ok_or("MDF record offset overflow.")?
            };
            let bytes = self.read_range(target.stream, position, target.record_size)?;
            let target = &self.groups[group];
            let values = target
                .channels
                .iter()
                .map(|c| c.value(&bytes, target.data_size, index))
                .collect::<Result<Vec<_>>>()?;
            let time = values[target.master]
                .number()
                .filter(|t| t.is_finite())
                .map(|t| {
                    let formatted = format!("{t:.9}");
                    formatted
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_owned()
                })
                .unwrap_or_else(|| "N/A".into());
            let row = if let Some(event) = target.event {
                target.can_row(event, time, &values, parser)?
            } else {
                std::iter::once(time)
                    .chain(
                        values
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| *i != target.master)
                            .map(|(_, v)| v.display()),
                    )
                    .collect()
            };
            rows.push(row);
        }
        Ok(Page {
            group,
            offset,
            total,
            rows,
        })
    }

    fn bytes(&mut self, offset: u64, count: usize) -> Result<Vec<u8>> {
        if offset
            .checked_add(count as u64)
            .filter(|&end| end <= self.length)
            .is_none()
        {
            return Err("Truncated MF4 block or out-of-range link.".into());
        }
        let mut data = vec![0; count];
        self.file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
        self.file.read_exact(&mut data).map_err(io_error)?;
        Ok(data)
    }

    fn block(&mut self, position: u64) -> Result<Block> {
        self.blocks_remaining = self
            .blocks_remaining
            .checked_sub(1)
            .ok_or("MF4 exceeds the block browsing limit.")?;
        if position < 64 || position % 8 != 0 {
            return Err("Invalid MDF block address.".into());
        }
        let header = self.bytes(position, 24)?;
        let length = u64_at(&header, 8)?;
        let count = u64_at(&header, 16)?;
        let link_bytes = count
            .checked_mul(8)
            .filter(|&s| s <= MAX_METADATA)
            .ok_or("Too many MDF block links.")?;
        if length < 24 + link_bytes
            || position
                .checked_add(length)
                .filter(|&end| end <= self.length)
                .is_none()
        {
            return Err("Truncated or invalid MDF block length.".into());
        }
        let bytes = self.bytes(position + 24, link_bytes as usize)?;
        let links = bytes
            .chunks_exact(8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            .collect();
        Ok(Block {
            id: header[..4].try_into().unwrap(),
            links,
            data: position + 24 + link_bytes,
            size: length - 24 - link_bytes,
        })
    }

    fn expected(&mut self, position: u64, id: &[u8; 4], links: usize, size: u64) -> Result<Block> {
        let block = self.block(position)?;
        if &block.id != id || block.links.len() < links || block.size < size {
            return Err(format!("Invalid {} block.", String::from_utf8_lossy(id)));
        }
        Ok(block)
    }

    fn payload(&mut self, block: &Block) -> Result<Vec<u8>> {
        if block.size > MAX_METADATA {
            return Err("MDF metadata block exceeds 1 MiB.".into());
        }
        self.metadata_remaining = self
            .metadata_remaining
            .checked_sub(block.size)
            .ok_or("MF4 metadata exceeds the 64 MiB browsing limit.")?;
        self.bytes(block.data, block.size as usize)
    }

    fn text(&mut self, position: u64) -> Result<String> {
        if position == 0 {
            return Ok(String::new());
        }
        let block = self.block(position)?;
        if ![b"##TX", b"##MD"].contains(&&block.id) {
            return Err("Invalid MDF text link.".into());
        }
        let bytes = self.payload(&block)?;
        let bytes = bytes.split(|&b| b == 0).next().unwrap_or_default();
        String::from_utf8(bytes.to_vec()).map_err(|_| "MDF text is not valid UTF-8.".into())
    }

    fn channels(
        &mut self,
        mut position: u64,
        channels: &mut Vec<Channel>,
        seen: &mut HashSet<u64>,
        depth: usize,
    ) -> Result<()> {
        if depth > 16 {
            return Err("MDF channel composition is too deep.".into());
        }
        while position != 0 {
            visit(seen, position)?;
            let block = self.expected(position, b"##CN", 8, 72)?;
            let bytes = self.payload(&block)?;
            if block.links[1] != 0 {
                let component = self.block(block.links[1])?;
                if &component.id != b"##CN" {
                    return Err("MDF array channels are not supported.".into());
                }
                self.channels(block.links[1], channels, seen, depth + 1)?;
            } else {
                let mut unit = self.text(block.links[6])?;
                let conversion = self.conversion(block.links[4], &mut unit)?;
                channels.push(Channel {
                    name: self.text(block.links[2])?,
                    unit,
                    channel_type: bytes[0],
                    sync_type: bytes[1],
                    data_type: bytes[2],
                    shift: bytes[3],
                    offset: u32_at(&bytes, 4)? as usize,
                    bits: u32_at(&bytes, 8)?,
                    flags: u32_at(&bytes, 12)?,
                    invalid_bit: u32_at(&bytes, 16)?,
                    conversion,
                });
                if channels.len() > MAX_CHANNELS {
                    return Err(
                        "MDF groups with more than 256 leaf channels are not supported.".into(),
                    );
                }
            }
            position = block.links[0];
        }
        Ok(())
    }

    fn conversion(&mut self, position: u64, unit: &mut String) -> Result<Conversion> {
        if position == 0 {
            return Ok(Conversion::Identity);
        }
        let block = self.expected(position, b"##CC", 4, 24)?;
        if unit.is_empty() {
            *unit = self.text(block.links[1])?;
        }
        let bytes = self.payload(&block)?;
        let values = u16_at(&bytes, 6)? as usize;
        if 24 + values * 8 > bytes.len() {
            return Err("Truncated MDF conversion parameters.".into());
        }
        Ok(match bytes[0] {
            0 => Conversion::Identity,
            1 if values == 2 => Conversion::Linear(f64_at(&bytes, 24)?, f64_at(&bytes, 32)?),
            2 if values == 6 => {
                let mut p = [0.; 6];
                for (i, value) in p.iter_mut().enumerate() {
                    *value = f64_at(&bytes, 24 + i * 8)?;
                }
                Conversion::Rational(p)
            }
            other => Conversion::Unsupported(other),
        })
    }

    fn data_blocks(
        &mut self,
        position: u64,
        stream: &mut Stream,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        let mut pending = vec![position];
        let mut seen = HashSet::new();
        while let Some(position) = pending.pop() {
            check_cancelled(cancelled)?;
            if position == 0 {
                continue;
            }
            visit(&mut seen, position)?;
            let block = self.block(position)?;
            match &block.id {
                b"##DT" => stream.segments.push(Segment {
                    start: stream.size,
                    data: block.data,
                    size: block.size,
                    stored: block.size,
                    compression: None,
                }),
                b"##DZ" => {
                    if block.size < 24 {
                        return Err("Truncated MDF compression header.".into());
                    }
                    let bytes = self.bytes(block.data, 24)?;
                    let size = u64_at(&bytes, 8)?;
                    let stored = u64_at(&bytes, 16)?;
                    let parameter = u32_at(&bytes, 4)?;
                    if &bytes[..2] != b"DT" || bytes[2] > 1 {
                        return Err(
                            "Only deflate and transposed-deflate MDF data are supported.".into(),
                        );
                    }
                    if size > MAX_INFLATED
                        || stored != block.size - 24
                        || (bytes[2] == 1 && parameter == 0)
                    {
                        return Err(
                            "Invalid compressed MDF block or decompressed block exceeds 128 MiB."
                                .into(),
                        );
                    }
                    stream.segments.push(Segment {
                        start: stream.size,
                        data: block.data + 24,
                        size,
                        stored,
                        compression: Some((bytes[2], parameter)),
                    });
                }
                b"##HL" if !block.links.is_empty() => {
                    pending.push(block.links[0]);
                    continue;
                }
                b"##DL" if !block.links.is_empty() => {
                    let bytes = self.payload(&block)?;
                    let count = u32_at(&bytes, 4)? as usize;
                    if count + 1 != block.links.len() {
                        return Err("Invalid MDF data list link count.".into());
                    }
                    pending.push(block.links[0]);
                    pending.extend(block.links[1..].iter().rev());
                    continue;
                }
                _ => {
                    return Err(format!(
                        "Unsupported MDF data block {}. Row-based MDF 4 data is required.",
                        String::from_utf8_lossy(&block.id)
                    ));
                }
            }
            stream.size = stream
                .size
                .checked_add(stream.segments.last().unwrap().size)
                .ok_or("MDF data size overflow.")?;
        }
        Ok(())
    }

    fn read_range(&mut self, stream: usize, start: u64, count: usize) -> Result<Vec<u8>> {
        let source = &self.streams[stream];
        if start
            .checked_add(count as u64)
            .filter(|&end| end <= source.size)
            .is_none()
        {
            return Err("Truncated MDF sample data.".into());
        }
        let mut index = source
            .segments
            .partition_point(|s| s.start <= start)
            .saturating_sub(1);
        let mut result = Vec::with_capacity(count);
        let mut position = start;
        while result.len() < count {
            let segment = self.streams[stream]
                .segments
                .get(index)
                .ok_or("Truncated MDF data list.")?
                .clone();
            if position < segment.start || position > segment.start + segment.size {
                return Err("Invalid MDF data list offsets.".into());
            }
            let offset = position - segment.start;
            let take = (segment.size - offset).min((count - result.len()) as u64) as usize;
            if let Some((kind, parameter)) = segment.compression {
                if self
                    .cache
                    .as_ref()
                    .is_none_or(|(key, _)| *key != segment.data)
                {
                    self.file
                        .seek(SeekFrom::Start(segment.data))
                        .map_err(io_error)?;
                    let compressed = (&mut self.file).take(segment.stored);
                    let mut decoder = ZlibDecoder::new(compressed);
                    let mut data = Vec::new();
                    data.try_reserve_exact(segment.size as usize)
                        .map_err(|e| format!("Cannot allocate MF4 decompression buffer: {e}"))?;
                    decoder
                        .by_ref()
                        .take(segment.size + 1)
                        .read_to_end(&mut data)
                        .map_err(io_error)?;
                    if data.len() as u64 != segment.size || decoder.total_in() != segment.stored {
                        return Err("Corrupt or truncated MF4 deflate data.".into());
                    }
                    if kind == 1 {
                        let width = parameter as usize;
                        let rows = data.len() / width;
                        let mut restored = vec![0; data.len()];
                        for column in 0..width.min(data.len()) {
                            for row in 0..rows {
                                restored[row * width + column] = data[column * rows + row];
                            }
                        }
                        restored[rows * width..].copy_from_slice(&data[rows * width..]);
                        data = restored;
                    }
                    self.cache = Some((segment.data, data));
                }
                result.extend_from_slice(
                    &self.cache.as_ref().unwrap().1[offset as usize..offset as usize + take],
                );
            } else {
                result.extend_from_slice(&self.bytes(segment.data + offset, take)?);
            }
            position += take as u64;
            index += 1;
        }
        Ok(result)
    }

    fn index_stream(
        &mut self,
        stream: usize,
        first: usize,
        id_size: usize,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        let groups = self.groups.len() - first;
        if groups == 0 {
            return Ok(());
        }
        if id_size == 0 {
            if groups != 1 {
                return Err("Multiple MDF groups require record IDs.".into());
            }
            let group = &self.groups[first];
            let expected = group
                .info
                .records
                .checked_mul(group.record_size as u64)
                .ok_or("MDF record count overflow.")?;
            if expected > self.streams[stream].size {
                return Err("MF4 sample data is truncated.".into());
            }
            return Ok(());
        }
        let mut ids = HashMap::new();
        for index in first..self.groups.len() {
            if ids.insert(self.groups[index].record_id, index).is_some() {
                return Err("Duplicate MDF record IDs.".into());
            }
            self.groups[index].positions = Some(Vec::new());
        }
        let mut position = 0;
        while position < self.streams[stream].size {
            check_cancelled(cancelled)?;
            if self.index_remaining == 0 {
                return Err("Unsorted MF4 exceeds the 16 million record index limit.".into());
            }
            let bytes = self.read_range(stream, position, id_size)?;
            let mut id = [0; 8];
            id[..id_size].copy_from_slice(&bytes);
            let &group = ids
                .get(&u64::from_le_bytes(id))
                .ok_or("Unknown MDF record ID.")?;
            position += id_size as u64;
            self.groups[group]
                .positions
                .as_mut()
                .unwrap()
                .push(position);
            position = position
                .checked_add(self.groups[group].record_size as u64)
                .filter(|&p| p <= self.streams[stream].size)
                .ok_or("Truncated unsorted MDF record.")?;
            self.index_remaining -= 1;
        }
        for group in &self.groups[first..] {
            if group.positions.as_ref().unwrap().len() as u64 != group.info.records {
                return Err("MDF record count does not match sample data.".into());
            }
        }
        Ok(())
    }
}

impl Channel {
    fn value(&self, record: &[u8], data_size: usize, index: u64) -> Result<Value> {
        if self.flags & 1 != 0
            || (self.flags & 2 != 0
                && record[data_size + self.invalid_bit as usize / 8]
                    & (1 << (self.invalid_bit % 8))
                    != 0)
        {
            return Ok(Value::Invalid);
        }
        if let Conversion::Unsupported(kind) = self.conversion {
            return Ok(Value::Text(format!("Unsupported conversion {kind}")));
        }
        let raw = if [3, 6].contains(&self.channel_type) {
            Value::Unsigned(index)
        } else {
            if self.channel_type != 0 && self.channel_type != 2 {
                return Ok(Value::Text(format!(
                    "Unsupported channel type {}",
                    self.channel_type
                )));
            }
            let count = (self.bits as usize + self.shift as usize).div_ceil(8);
            let bytes = &record[self.offset..self.offset + count];
            match self.data_type {
                0..=3 if self.bits > 0 && self.bits <= 64 && self.shift < 8 => {
                    let mut value = 0u128;
                    if self.data_type % 2 == 0 {
                        for (i, &b) in bytes.iter().enumerate() {
                            value |= (b as u128) << (i * 8);
                        }
                    } else {
                        for &b in bytes {
                            value = (value << 8) | b as u128;
                        }
                    }
                    let value = ((value >> self.shift) & ((1u128 << self.bits) - 1)) as u64;
                    if self.data_type < 2 {
                        Value::Unsigned(value)
                    } else {
                        Value::Signed(((value << (64 - self.bits)) as i64) >> (64 - self.bits))
                    }
                }
                4..=5 if self.shift == 0 && [32, 64].contains(&self.bits) => {
                    let value = if self.bits == 32 {
                        let bytes = bytes.try_into().unwrap();
                        (if self.data_type == 4 {
                            f32::from_le_bytes(bytes)
                        } else {
                            f32::from_be_bytes(bytes)
                        }) as f64
                    } else {
                        let bytes = bytes.try_into().unwrap();
                        if self.data_type == 4 {
                            f64::from_le_bytes(bytes)
                        } else {
                            f64::from_be_bytes(bytes)
                        }
                    };
                    Value::Float(value)
                }
                6 if self.shift == 0 && self.bits % 8 == 0 => Value::Text(
                    bytes
                        .iter()
                        .take_while(|&&b| b != 0)
                        .map(|&b| b as char)
                        .collect(),
                ),
                7 if self.shift == 0 && self.bits % 8 == 0 => Value::Text(
                    String::from_utf8_lossy(bytes.split(|&b| b == 0).next().unwrap_or_default())
                        .into(),
                ),
                10 if self.shift == 0 && self.bits % 8 == 0 => Value::Bytes(bytes.to_vec()),
                _ => Value::Text(format!(
                    "Unsupported data type {} / {} bits",
                    self.data_type, self.bits
                )),
            }
        };
        Ok(match (&self.conversion, raw.number()) {
            (Conversion::Linear(offset, factor), Some(x)) => Value::Float(offset + factor * x),
            (Conversion::Rational(p), Some(x)) => {
                Value::Float((p[0] * x * x + p[1] * x + p[2]) / (p[3] * x * x + p[4] * x + p[5]))
            }
            _ => raw,
        })
    }
}

impl Group {
    fn can_row(
        &self,
        event: &str,
        time: String,
        values: &[Value],
        parser: &CanParser,
    ) -> Result<Vec<String>> {
        let field = |name: &str| {
            self.channels
                .iter()
                .position(|c| c.name == format!("{event}.{name}"))
                .map(|i| &values[i])
        };
        let number = |name: &str, default: u64| -> Result<u64> {
            field(name).map_or(Ok(default), Value::unsigned)
        };
        let original_id = number("ID", 0)?;
        let id = u32::try_from(original_id & 0x1FFF_FFFF).unwrap();
        let extended = number("IDE", 0)? != 0 || original_id & 0x8000_0000 != 0;
        let remote = event == "CAN_RemoteFrame";
        let error = event == "CAN_ErrorFrame";
        let fd = event == "CANFD_DataFrame" || number("EDL", 0)? != 0;
        let dlc = number("DLC", 0)?;
        let payload = match field("DataBytes") {
            Some(Value::Bytes(bytes)) => bytes.as_slice(),
            None if remote => &[],
            _ => return Err("CAN event has no valid DataBytes channel.".into()),
        };
        let count = if remote {
            0
        } else {
            number("DataLength", dlc)? as usize
        };
        if count > payload.len() || count > 64 || dlc > 15 {
            return Err("Invalid CAN payload length in MF4.".into());
        }
        let payload = &payload[..count];
        let bus = number("BusChannel", 0)?;
        let interface = if self.interface.is_empty() {
            format!("CAN {bus}")
        } else {
            format!("{} / bus {bus}", self.interface)
        };
        let mut flags = vec![
            if extended { "EXT" } else { "STD" },
            if number("Dir", 0)? == 0 { "RX" } else { "TX" },
        ];
        if remote {
            flags.push("RTR");
        }
        if error {
            flags.push("ERR");
        }
        if fd {
            flags.push("FD");
        }
        if number("BRS", 0)? != 0 {
            flags.push("BRS");
        }
        if number("ESI", 0)? != 0 {
            flags.push("ESI");
        }
        let frame = if remote {
            CanFrame::new_remote(id, dlc.min(8) as usize, extended)
        } else if error {
            CanFrame::new_error(id)
        } else if extended {
            CanFrame::new_eff(id, payload)
        } else {
            CanFrame::new(id, payload)
        };
        let decoded = frame
            .ok()
            .and_then(|frame| parser.decode_for_recording(&frame))
            .map(|(name, signals)| {
                signals
                    .into_iter()
                    .map(|s| {
                        format!(
                            "{name}.{} = {}{}{}",
                            s.name,
                            s.value,
                            if s.unit.is_empty() { "" } else { " " },
                            s.unit
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        Ok(vec![
            time,
            interface,
            format!("{id:X}"),
            flags.join(" "),
            dlc.to_string(),
            hex(payload),
            decoded,
        ])
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
fn io_error(error: std::io::Error) -> String {
    format!("MF4 read failed: {error}")
}
fn check_cancelled(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err("MF4 operation cancelled.".into())
    } else {
        Ok(())
    }
}
fn visit(seen: &mut HashSet<u64>, position: u64) -> Result<()> {
    if seen.len() >= MAX_BLOCKS || !seen.insert(position) {
        Err("Cyclic MDF links or excessive block count.".into())
    } else {
        Ok(())
    }
}
fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or("Truncated MDF field.")?
            .try_into()
            .unwrap(),
    ))
}
fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or("Truncated MDF field.")?
            .try_into()
            .unwrap(),
    ))
}
fn u64_at(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .ok_or("Truncated MDF field.")?
            .try_into()
            .unwrap(),
    ))
}
fn f64_at(bytes: &[u8], offset: usize) -> Result<f64> {
    Ok(f64::from_bits(u64_at(bytes, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{recording::Recorder, trace_filter::TraceFilter};
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU64;

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temporary(PathBuf);
    impl Temporary {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "cyder reader {} {}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn native(&self) -> PathBuf {
            let mut recorder = Recorder::start(&self.0, 0, "mf4", CanParser::new()).unwrap();
            let mut frame = CanFrame::new(0x123, &[1, 2, 3]).unwrap();
            frame.set_timestamp(Some(12345));
            recorder
                .record(&frame, "can0", &TraceFilter::default())
                .unwrap();
            recorder.finish().unwrap();
            recorder.paths[0].clone()
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            for entry in std::fs::read_dir(&self.0).unwrap() {
                let _ = std::fs::remove_file(entry.unwrap().path());
            }
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    #[test]
    fn rejects_corrupt_links_counts_and_truncated_files_without_panicking() {
        let directory = Temporary::new();
        let path = directory.native();
        let original = std::fs::read(&path).unwrap();
        let dg = u64_at(&original, 88).unwrap() as usize;
        let cg = u64_at(&original, dg + 32).unwrap() as usize;
        let cn = u64_at(&original, cg + 32).unwrap() as usize;
        let cancelled = AtomicBool::new(false);
        let mutations = [
            (dg + 24, dg as u64), // DG cycle
            (cn + 24, cn as u64), // CN cycle
            (dg + 40, u64::MAX),  // Out-of-file data link
            (cg + 80, u64::MAX),  // Sample count overflow
            (dg + 8, u64::MAX),   // Block size overflow
        ];
        for (position, value) in mutations {
            let mut bytes = original.clone();
            bytes[position..position + 8].copy_from_slice(&value.to_le_bytes());
            std::fs::write(&path, bytes).unwrap();
            assert!(
                Capture::open(&path, &cancelled).is_err(),
                "accepted mutation at {position}"
            );
        }
        for length in [0, 8, 63, 100, original.len() - 1] {
            std::fs::write(&path, &original[..length]).unwrap();
            assert!(Capture::open(&path, &cancelled).is_err());
        }
        std::fs::write(&path, &original).unwrap();
        let mut capture = Capture::open(&path, &cancelled).unwrap();
        assert!(
            capture
                .page(10, 0, 500, &CanParser::new(), &cancelled)
                .is_err()
        );
        assert!(
            capture
                .page(0, u64::MAX, 500, &CanParser::new(), &cancelled)
                .unwrap()
                .rows
                .is_empty()
        );
        assert_eq!(
            capture
                .page(0, 0, 500, &CanParser::new(), &cancelled)
                .unwrap()
                .rows[0][0],
            "0.012345"
        );
        cancelled.store(true, Ordering::Relaxed);
        assert!(
            capture
                .page(0, 0, 1, &CanParser::new(), &cancelled)
                .is_err()
        );
        assert!(Capture::open(&path, &cancelled).is_err());
    }

    #[test]
    fn unsorted_record_ids_select_the_correct_group_and_page() {
        let directory = Temporary::new();
        let path = directory.native();
        let mut bytes = std::fs::read(&path).unwrap();
        let dg = u64_at(&bytes, 88).unwrap() as usize;
        let cg = u64_at(&bytes, dg + 32).unwrap() as usize;
        let dl = u64_at(&bytes, dg + 40).unwrap() as usize;
        let dt = u64_at(&bytes, dl + 32).unwrap() as usize;
        let record = bytes[dt + 24..dt + 49].to_vec();
        bytes.resize(bytes.len().div_ceil(8) * 8, 0);
        let other = bytes.len();
        let cg_copy = bytes[cg..cg + 104].to_vec();
        bytes.extend_from_slice(&cg_copy);
        bytes[cg + 24..cg + 32].copy_from_slice(&(other as u64).to_le_bytes());
        for (group, id, count) in [(cg, 7u64, 3u64), (other, 8, 2)] {
            bytes[group + 72..group + 80].copy_from_slice(&id.to_le_bytes());
            bytes[group + 80..group + 88].copy_from_slice(&count.to_le_bytes());
        }
        bytes[dg + 56] = 1; // One-byte record IDs.
        let data_block = bytes.len();
        bytes.extend_from_slice(b"##DT\0\0\0\0");
        bytes.extend_from_slice(&(24u64 + 26 * 5).to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        for index in 0..5 {
            bytes.push(if index % 2 == 0 { 7 } else { 8 });
            let mut frame = record.clone();
            frame[..8].copy_from_slice(&((index + 1) as f64 * 0.001).to_le_bytes());
            frame[9..13].copy_from_slice(&(0x123 + index as u32).to_le_bytes());
            bytes.extend_from_slice(&frame);
        }
        bytes[dg + 40..dg + 48].copy_from_slice(&(data_block as u64).to_le_bytes());
        std::fs::write(&path, &bytes).unwrap();
        let cancelled = AtomicBool::new(false);
        let mut capture = Capture::open(&path, &cancelled).unwrap();
        assert_eq!(capture.info.groups.len(), 2);
        let page = capture
            .page(0, 1, 500, &CanParser::new(), &cancelled)
            .unwrap();
        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.rows[0][2], "125");
        assert_eq!(page.rows[1][2], "127");
        let page = capture
            .page(1, 0, 500, &CanParser::new(), &cancelled)
            .unwrap();
        assert_eq!(page.rows[0][0], "0.002");
        assert_eq!(page.rows[1][2], "126");
        drop(capture);
        bytes[data_block + 24] = 255;
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            Capture::open(&path, &cancelled)
                .err()
                .unwrap()
                .contains("Unknown MDF record ID")
        );
    }

    #[test]
    fn independent_uncompressed_deflate_and_transposed_files() {
        let Some(python) = std::env::var_os("CYDERVIS_MF4_VERIFY_PYTHON") else {
            return;
        };
        let directory = Temporary::new();
        let output = std::process::Command::new(python)
            .arg("tests/make_mf4_fixtures.py")
            .arg(&directory.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let cancelled = AtomicBool::new(false);
        let mut parser = CanParser::new();
        parser
            .open_dbc("../godot/tests/transmit.dbc".into())
            .unwrap();
        for compression in 0..=2 {
            let mut raw = Capture::open(
                &directory.0.join(format!("raw-{compression}.mf4")),
                &cancelled,
            )
            .unwrap();
            assert_eq!(raw.info.groups.iter().map(|g| g.records).sum::<u64>(), 2054);
            let data = raw
                .info
                .groups
                .iter()
                .position(|g| g.records == 2052)
                .unwrap();
            let page = raw.page(data, 2000, 500, &parser, &cancelled).unwrap();
            assert_eq!(page.rows.len(), 52);
            assert_eq!(page.rows[0][2], "10302");
            assert_eq!(page.rows[0][5], "6F 12 83 3A 0B 01 00 00");
            assert!(page.rows[0][6].contains("SetComplementaryFilter.odr = 11"));
            assert_eq!(page.rows[50][5], "");
            assert_eq!(page.rows[51][4], "9");
            assert_eq!(page.rows[51][5], "00 01 02 03 04 05 06 07 08 09 0A 0B");
            assert!(page.rows[51][3].contains("FD BRS ESI"));
            let mut flags = Vec::new();
            for group in 0..raw.groups.len() {
                if group != data {
                    flags.push(
                        raw.page(group, 0, 1, &parser, &cancelled).unwrap().rows[0][3].clone(),
                    );
                }
            }
            assert!(flags.iter().any(|f| f.contains("ERR")));
            assert!(flags.iter().any(|f| f.contains("RTR")));
            let mut signals = Capture::open(
                &directory.0.join(format!("signals-{compression}.mf4")),
                &cancelled,
            )
            .unwrap();
            assert!(
                signals.info.groups[0]
                    .columns
                    .contains(&"Motor.temperature (°C)".into())
            );
            let page = signals.page(0, 0, 20, &parser, &cancelled).unwrap();
            assert_eq!(
                page.rows[0],
                [
                    "0.01",
                    "-40",
                    "-1234",
                    "0.12345678912345678",
                    "18446744073709551615"
                ]
            );
            assert_eq!(page.rows[10][3], "N/A");
            assert!((page.rows[19][1].parse::<f64>().unwrap() + 38.1).abs() < 1e-12);
            assert_eq!(
                signals
                    .page(0, 2049, 500, &parser, &cancelled)
                    .unwrap()
                    .rows
                    .len(),
                1
            );
        }
        assert!(
            Capture::open(&directory.0.join("empty.mf4"), &cancelled)
                .unwrap()
                .info
                .groups
                .is_empty()
        );
        assert!(Capture::open(&directory.0.join("broken.mf4"), &cancelled).is_err());
    }
}
