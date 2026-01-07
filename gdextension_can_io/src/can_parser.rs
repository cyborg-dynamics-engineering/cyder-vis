///
/// can_parser.rs
///
/// Parses CanFrames into Godot Variant Arrays.
/// Can optionally utilise a CAN DBC file to parse the raw data into named items in the Godot Arrays.
///
use crate::{CanEntry, CanId};
use can_dbc::{ByteOrder, Dbc, DbcError};
use core::panic;
use crosscan::can::CanFrame;
use godot::builtin::{GString, VariantArray};
use godot::prelude::*;
use std::collections::HashMap;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    CanDbc(DbcError),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

// Conversion between the different MessageId structs
mod dbc_helpers {
    use crosscan::can::CanFrame;
    pub fn get_message_id(frame: &CanFrame) -> can_dbc::MessageId {
        use can_dbc::MessageId;
        if frame.is_extended() {
            return MessageId::Extended(frame.id());
        } else {
            return MessageId::Standard(frame.id() as u16);
        }
    }
}

pub struct CanParser {
    dbc: Option<Dbc>,
}

impl CanParser {
    pub fn new() -> Self {
        return Self { dbc: None };
    }

    /// Loads a new DBC file into the CanParser for future deserialisation
    pub fn open_dbc(&mut self, file_path: String) -> Result<(), Error> {
        let data = std::fs::read_to_string(file_path.clone())?;
        match Dbc::try_from(data.as_str()) {
            Ok(dbc) => {
                self.dbc = Some(dbc);
                Ok(())
            }
            Err(e) => Err(Error::CanDbc(e)),
        }
    }

    /// Clears the DBC file if one is currently loaded in
    pub fn clear_dbc(&mut self) {
        self.dbc = None
    }

    /// Parses a set of CanDataFrames into a table of Godot CAN entries. Will optionally use a DBC for deserialisation if provided.
    pub fn parse_can_table(&self, can_entries: &HashMap<CanId, CanEntry>) -> Array<Variant> {
        let mut godot_can_table = VariantArray::new();

        for (_, entry) in can_entries.iter() {
            let godot_can_entry = &self.parse_can_entry(entry).to_variant();
            godot_can_table.push(godot_can_entry);
        }

        godot_can_table
    }

    // Parses an individual CAN frame as a String if it exists in the DBC file
    pub fn parse_frame_as_string(&self, frame: &CanFrame) -> Option<String> {
        let mut frame_string = String::new();

        // Query if a dbc entry exists for this id
        if let Some(dbc) = &self.dbc {
            let query_id = dbc_helpers::get_message_id(frame);

            // If dbc attempt to deserialize
            if let Some(message_info) = dbc.messages.iter().find(|m| m.id == query_id) {
                for signal in &message_info.signals {
                    frame_string.push_str("  ");
                    frame_string.push_str(&signal.name.clone());
                    frame_string.push_str(" -> ");

                    let mut bytes = frame.data().to_vec();
                    if signal.byte_order == ByteOrder::BigEndian {
                        CanParser::reverse_bit_order(&mut bytes);
                    }

                    frame_string.push_str(&self.deserialise_signal_value(
                        message_info,
                        signal,
                        bytes,
                    ));
                    frame_string.push_str("\n");
                }
                return Some(frame_string);
            }

            // CAN ID does not exist in DBC file
            return None;
        }

        // No DBC file exists
        None
    }

    /// Parses a given CanEntry into a Godot CAN entry. Will optionally use a DBC for deserialisation if provided.
    fn parse_can_entry(&self, can_entry: &CanEntry) -> Array<Variant> {
        let mut godot_can_entry = VariantArray::new();

        godot_can_entry
            .push(&GString::from(format!("{:?}", can_entry.timestamps.back())).to_variant());
        godot_can_entry.push(&GString::from(format!("{:?}", can_entry.freq_hz)).to_variant());
        godot_can_entry.push(&GString::from(format!("{:?}", can_entry.frame.id())).to_variant());

        // Query if a dbc entry exists for this id
        if let Some(dbc) = &self.dbc {
            let query_id = dbc_helpers::get_message_id(&can_entry.frame);

            // if dbc attempt to deserialize
            if let Some(message_info) = dbc.messages.iter().find(|m| m.id == query_id) {
                godot_can_entry.push(&GString::from(message_info.name.clone()).to_variant());

                // TODO: Check if can deserialize
                godot_can_entry = self.deserialise_dbc_data(
                    godot_can_entry,
                    can_entry.frame.clone(),
                    message_info,
                );
            } else {
                godot_can_entry.push(&GString::from("").to_variant()); // Empty msg name to indicate no definition in the DBC
                godot_can_entry =
                    Self::deserialise_unknown_data(godot_can_entry, can_entry.frame.clone());
            }
        } else {
            // otherwise don't deserialize dbc
            godot_can_entry.push(&GString::from("").to_variant()); // Empty msg name to indicate no definition in the DBC
            godot_can_entry =
                Self::deserialise_unknown_data(godot_can_entry, can_entry.frame.clone());
        }

        // The last element indicates to Godot whether the frame is Extended
        godot_can_entry
            .push(&GString::from(format!("{:?}", can_entry.frame.is_extended())).to_variant());

        godot_can_entry
    }

    /// Deserialises and appends the data from the CAN frame into the Godot CAN entry
    fn deserialise_dbc_data(
        &self,
        mut godot_can_entry: Array<Variant>,
        frame: CanFrame,
        message_info: &can_dbc::Message,
    ) -> Array<Variant> {
        for signal in &message_info.signals {
            godot_can_entry.push(&GString::from(signal.name.clone()).to_variant());

            let dbc = match &self.dbc {
                Some(table) => table,
                None => {
                    panic!("Attempted to deserialise data using DBC when no DBC table has been set")
                }
            };

            let value = match dbc
                .extended_value_type_for_signal(message_info.id, signal.name.as_str())
                .unwrap_or(&can_dbc::SignalExtendedValueType::SignedOrUnsignedInteger)
            {
                can_dbc::SignalExtendedValueType::SignedOrUnsignedInteger => {
                    let start_bit = usize::try_from(signal.start_bit).unwrap();
                    let length = usize::try_from(signal.size).unwrap();
                    match signal.value_type {
                        can_dbc::ValueType::Signed => {
                            CanParser::extract_bits_i64(
                                frame.data(),
                                start_bit,
                                length,
                                signal.byte_order,
                            ) as f64
                                * signal.factor
                                + signal.offset
                        }
                        can_dbc::ValueType::Unsigned => {
                            CanParser::extract_bits_u64(
                                frame.data(),
                                start_bit,
                                length,
                                signal.byte_order,
                            ) as f64
                                * signal.factor
                                + signal.offset
                        }
                    }
                }
                can_dbc::SignalExtendedValueType::IEEEfloat32Bit => {
                    let start_bit = usize::try_from(signal.start_bit).unwrap();
                    let raw_value =
                        CanParser::extract_bits_u64(frame.data(), start_bit, 32, signal.byte_order)
                            as u32;

                    f32::from_bits(raw_value) as f64 * signal.factor + signal.offset
                }
                can_dbc::SignalExtendedValueType::IEEEdouble64bit => {
                    let start_bit = usize::try_from(signal.start_bit).unwrap();
                    let raw_value =
                        CanParser::extract_bits_u64(frame.data(), start_bit, 64, signal.byte_order);

                    f64::from_bits(raw_value) * signal.factor + signal.offset
                }
            };

            let formatted_value = format!("{:.15}", value)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string();

            // Add asterix if values falls outside bounds
            let formatted_value_with_bounds = format!(
                "{}{}",
                formatted_value,
                match value < signal.min || value > signal.max {
                    true => "*",
                    false => "",
                }
            );
            let mut bytes = frame.data().to_vec();
            if signal.byte_order == ByteOrder::BigEndian {
                CanParser::reverse_bit_order(&mut bytes);
            }

            godot_can_entry.push(
                &GString::from(self.deserialise_signal_value(message_info, signal, bytes))
                    .to_variant(),
            );
        }
        godot_can_entry
    }

    // Deserialises the value of a given signal
    fn deserialise_signal_value(
        &self,
        message_info: &can_dbc::Message,
        signal: &can_dbc::Signal,
        bytes: Vec<u8>,
    ) -> String {
        let dbc = match &self.dbc {
            Some(table) => table,
            None => {
                panic!("Attempted to deserialise data using DBC when no DBC table has been set")
            }
        };

        let value = match dbc
            .extended_value_type_for_signal(message_info.id, signal.name.as_str())
            .unwrap_or(&can_dbc::SignalExtendedValueType::SignedOrUnsignedInteger)
        {
            can_dbc::SignalExtendedValueType::SignedOrUnsignedInteger => {
                let start_bit = usize::try_from(signal.start_bit).unwrap();
                let length = usize::try_from(signal.size).unwrap();
                match signal.value_type {
                    can_dbc::ValueType::Signed => {
                        CanParser::extract_bits_i64(bytes, start_bit, length) as f64 * signal.factor
                            + signal.offset
                    }
                    can_dbc::ValueType::Unsigned => {
                        CanParser::extract_bits_u64(bytes, start_bit, length) as f64 * signal.factor
                            + signal.offset
                    }
                }
            }
            can_dbc::SignalExtendedValueType::IEEEfloat32Bit => {
                let start_bit = usize::try_from(signal.start_bit).unwrap();
                let raw_value = CanParser::extract_bits_u64(bytes, start_bit, 32) as u32;

                f32::from_bits(raw_value) as f64 * signal.factor + signal.offset
            }
            can_dbc::SignalExtendedValueType::IEEEdouble64bit => {
                let start_bit = usize::try_from(signal.start_bit).unwrap();
                let raw_value = CanParser::extract_bits_u64(bytes, start_bit, 64);

                f64::from_bits(raw_value) * signal.factor + signal.offset
            }
        };

        let formatted_value = format!("{:?}", value)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();

        // Add asterix if values falls outside bounds
        let formatted_value_with_bounds = format!(
            "{}{}",
            formatted_value,
            match value < signal.min || value > signal.max {
                true => "*",
                false => "",
            }
        );
        formatted_value_with_bounds
    }

    /// Deserialises and appends the raw byte data from the CAN frame to the Godot CAN entry
    fn deserialise_unknown_data(
        mut godot_can_entry: Array<Variant>,
        frame: CanFrame,
    ) -> Array<Variant> {
        for byte in frame.data() {
            godot_can_entry.push(&GString::from(format!("{:?}", byte)).to_variant())
        }
        godot_can_entry
    }

    // Extracts a u64 value from a data vector given the start bit and length. Assumes little-endian bit representation.
    fn extract_bits_u64(
        bytes: &[u8],
        start_bit: usize,
        length: usize,
        byte_order: ByteOrder,
    ) -> u64 {
        assert!(bytes.len() <= 8, "Input slice must a maximum of 8 bytes");
        assert!(length <= bytes.len() * 8, "Signal length exceeds data size");

        match byte_order {
            ByteOrder::LittleEndian => {
                assert!(
                    start_bit + length <= bytes.len() * 8,
                    "Out of bounds bit extraction"
                );

                let mut bytes_buf = [0u8; 8];
                bytes_buf[..bytes.len()].copy_from_slice(bytes);

                let value = u64::from_le_bytes(bytes_buf);
                let mask = if length == 64 {
                    u64::MAX
                } else {
                    (1u64 << length) - 1
                };
                (value >> start_bit) & mask
            }
            ByteOrder::BigEndian => {
                // DBC Motorola ("big endian") signals are not correctly handled by reversing bytes/bits.
                // Here we interpret start_bit as the MSB position in DBC bit numbering (bit 0 = LSB of byte0),
                // and walk toward the LSB in Motorola order:
                //   if bit == 0 -> jump to next byte's bit 7 (pos += 15)
                //   else        -> pos -= 1
                //
                // We build the integer MSB-first.
                let total_bits = bytes.len() * 8;
                assert!(start_bit < total_bits, "Out of bounds bit extraction");

                let mut pos = start_bit;
                let mut out: u64 = 0;

                for i in 0..length {
                    let byte_i = pos / 8;
                    let bit_i = pos % 8;
                    assert!(byte_i < bytes.len(), "Out of bounds bit extraction");

                    let bit = (bytes[byte_i] >> bit_i) & 1;
                    out = (out << 1) | (bit as u64);

                    // Don't advance past the final extracted bit
                    if i + 1 == length {
                        break;
                    }

                    // Advance toward LSB in Motorola order
                    pos = if bit_i == 0 { pos + 15 } else { pos - 1 };
                    assert!(pos < total_bits, "Out of bounds bit extraction");
                }

                out
            }
        }
    }
    // Extracts an i64 value from a data vector given the start bit and length. Assumes little-endian bit representation.
    fn extract_bits_i64(
        bytes: &[u8],
        start_bit: usize,
        length: usize,
        byte_order: ByteOrder,
    ) -> i64 {
        let mut value = CanParser::extract_bits_u64(bytes, start_bit, length, byte_order) as i64;

        // Sign extend if the most significant bit of the extracted value is set (two's complement)
        if value & (1 << (length - 1)) != 0 {
            value = value | (!0 << length);
        }

        value
    }
}
