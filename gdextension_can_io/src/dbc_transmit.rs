use can_dbc::{
    ByteOrder, Dbc, Message, MessageId, MultiplexIndicator, Signal, SignalExtendedValueType,
    ValueType,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct MessageDefinition {
    id: u32,
    extended: bool,
    name: String,
    signals: Vec<SignalDefinition>,
    error: String,
}

#[derive(Serialize)]
struct SignalDefinition {
    name: String,
    kind: String,
    unit: String,
    min: f64,
    max: f64,
    initial: String,
}

pub fn definitions(dbc: &Dbc) -> Vec<MessageDefinition> {
    dbc.messages
        .iter()
        .map(|message| MessageDefinition {
            id: message.id.raw() & 0x1fff_ffff,
            extended: matches!(message.id, MessageId::Extended(_)),
            name: message.name.clone(),
            error: validate_message(dbc, message).err().unwrap_or_default(),
            signals: message
                .signals
                .iter()
                .map(|signal| {
                    let kind = match signal_type(dbc, message, signal) {
                        SignalExtendedValueType::IEEEfloat32Bit => "f32".into(),
                        SignalExtendedValueType::IEEEdouble64bit => "f64".into(),
                        _ if signal.factor == 1.0
                            && signal.offset == 0.0
                            && signal.value_type == ValueType::Unsigned =>
                        {
                            format!("hex{}", signal.size)
                        }
                        _ => format!("number{}", signal.size),
                    };
                    let initial = if signal.min < signal.max {
                        0.0f64.clamp(signal.min, signal.max)
                    } else {
                        0.0
                    };
                    SignalDefinition {
                        name: signal.name.clone(),
                        kind,
                        unit: signal.unit.clone(),
                        min: signal.min,
                        max: signal.max,
                        initial: initial.to_string(),
                    }
                })
                .collect(),
        })
        .collect()
}

fn signal_type<'a>(
    dbc: &'a Dbc,
    message: &Message,
    signal: &Signal,
) -> &'a SignalExtendedValueType {
    dbc.extended_value_type_for_signal(message.id, &signal.name)
        .unwrap_or(&SignalExtendedValueType::SignedOrUnsignedInteger)
}

pub fn bit_positions(signal: &Signal, bytes: usize) -> Result<Vec<usize>, String> {
    if signal.size == 0 || signal.size > 64 || signal.start_bit >= (bytes * 8) as u64 {
        return Err(format!("{}: invalid signal layout", signal.name));
    }
    let mut position = signal.start_bit as usize;
    let mut positions = Vec::new();
    for _ in 0..signal.size {
        if position >= bytes * 8 {
            return Err(format!("{}: signal exceeds payload length", signal.name));
        }
        positions.push(position);
        position = match signal.byte_order {
            ByteOrder::LittleEndian => position + 1,
            ByteOrder::BigEndian if position % 8 == 0 => position + 15,
            ByteOrder::BigEndian => position - 1,
        };
    }
    Ok(positions)
}

fn validate_message(dbc: &Dbc, message: &Message) -> Result<(), String> {
    if message.size > 8 {
        return Err("CAN FD payloads are not supported by this backend.".into());
    }
    if matches!(message.id, MessageId::Standard(id) if id > 0x7ff) {
        return Err("Invalid standard ID. Extended DBC IDs must include bit 31.".into());
    }
    let mut used = [false; 64];
    for signal in &message.signals {
        if signal.multiplexer_indicator != MultiplexIndicator::Plain {
            return Err(
                "Multiplexed transmission is not supported; use raw hex for this message.".into(),
            );
        }
        let size = match signal_type(dbc, message, signal) {
            SignalExtendedValueType::IEEEfloat32Bit => 32,
            SignalExtendedValueType::IEEEdouble64bit => 64,
            _ => signal.size,
        };
        if size != signal.size
            || !signal.factor.is_finite()
            || signal.factor == 0.0
            || !signal.offset.is_finite()
        {
            return Err(format!("{}: invalid type or scaling", signal.name));
        }
        for position in bit_positions(signal, message.size as usize)? {
            if used[position] {
                return Err("Overlapping signals require raw hex transmission.".into());
            }
            used[position] = true;
        }
    }
    Ok(())
}

pub fn encode(dbc: &Dbc, id: u32, extended: bool, inputs: &[String]) -> Result<Vec<u8>, String> {
    let message_id = if extended {
        MessageId::Extended(id)
    } else {
        MessageId::Standard(u16::try_from(id).map_err(|_| "Invalid standard ID")?)
    };
    let message = dbc
        .messages
        .iter()
        .find(|message| message.id == message_id)
        .ok_or("Message not found in the loaded DBC")?;
    validate_message(dbc, message)?;
    if inputs.len() != message.signals.len() {
        return Err("Signal fields changed; select the DBC message again.".into());
    }
    let mut bytes = vec![0u8; message.size as usize];
    for (signal, input) in message.signals.iter().zip(inputs) {
        let integer_type = matches!(
            signal_type(dbc, message, signal),
            SignalExtendedValueType::SignedOrUnsignedInteger
        );
        let exact_integer = if integer_type && signal.factor == 1.0 && signal.offset == 0.0 {
            let input = input.trim();
            if let Some(hex) = input
                .strip_prefix("0x")
                .or_else(|| input.strip_prefix("0X"))
            {
                Some(
                    i128::from_str_radix(hex, 16)
                        .map_err(|_| format!("{}: invalid hex integer", signal.name))?,
                )
            } else {
                Some(input.parse::<i128>().map_err(|_| {
                    format!("{}: enter an integer or 0x-prefixed hex value", signal.name)
                })?)
            }
        } else {
            None
        };
        let physical = match exact_integer {
            Some(value) => value as f64,
            None => input
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("{}: enter a numeric value", signal.name))?,
        };
        if !physical.is_finite()
            || (signal.min < signal.max && (physical < signal.min || physical > signal.max))
        {
            return Err(format!(
                "{}: value must be finite and within [{}, {}]",
                signal.name, signal.min, signal.max
            ));
        }
        let raw = (physical - signal.offset) / signal.factor;
        let bits = match signal_type(dbc, message, signal) {
            SignalExtendedValueType::IEEEfloat32Bit => {
                let value = raw as f32;
                if !value.is_finite() {
                    return Err(format!("{}: float32 overflow", signal.name));
                }
                u64::from(value.to_bits())
            }
            SignalExtendedValueType::IEEEdouble64bit => {
                if !raw.is_finite() {
                    return Err(format!("{}: float64 overflow", signal.name));
                }
                raw.to_bits()
            }
            _ => {
                let value = if let Some(value) = exact_integer {
                    value
                } else {
                    if !raw.is_finite() || (raw - raw.round()).abs() > 1e-6 {
                        return Err(format!(
                            "{}: value cannot be represented with the DBC scaling",
                            signal.name
                        ));
                    }
                    raw.round() as i128
                };
                let (min, max) = if signal.value_type == ValueType::Signed {
                    (
                        -(1i128 << (signal.size - 1)),
                        (1i128 << (signal.size - 1)) - 1,
                    )
                } else {
                    (0, (1i128 << signal.size) - 1)
                };
                if value < min || value > max {
                    return Err(format!(
                        "{}: integer exceeds {} bits",
                        signal.name, signal.size
                    ));
                }
                value as u64
            }
        };
        let positions = bit_positions(signal, bytes.len())?;
        for (index, position) in positions.into_iter().enumerate() {
            let source = match signal.byte_order {
                ByteOrder::LittleEndian => index,
                ByteOrder::BigEndian => signal.size as usize - 1 - index,
            };
            bytes[position / 8] |= (((bits >> source) & 1) as u8) << (position % 8);
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dbc(messages: &str) -> Dbc {
        Dbc::try_from(format!("VERSION \"\"\nNS_ :\nBS_:\nBU_: PC\n{messages}\n").as_str()).unwrap()
    }

    #[test]
    fn encodes_float_configuration_and_bounds() {
        let dbc = dbc(
            "BO_ 2147549954 SetFilter: 8 PC\n SG_ alpha : 0|32@1+ (1,0) [0.001|0.1] \"\" PC\n SG_ odr : 32|8@1+ (1,0) [0|11] \"\" PC\n SG_ output : 40|8@1+ (1,0) [0|1] \"\" PC\nSIG_VALTYPE_ 2147549954 alpha : 1;",
        );
        assert_eq!(
            encode(
                &dbc,
                0x10302,
                true,
                &["0.001".into(), "0x0b".into(), "1".into()]
            )
            .unwrap(),
            [0x6f, 0x12, 0x83, 0x3a, 11, 1, 0, 0]
        );
        assert!(encode(&dbc, 0x10302, true, &["0.2".into(), "1".into(), "1".into()]).is_err());
        assert!(encode(&dbc, 0x10302, true, &["nan".into(), "1".into(), "1".into()]).is_err());
        assert!(encode(&dbc, 0x10302, false, &[]).is_err());
        assert_eq!(definitions(&dbc)[0].signals[0].kind, "f32");
    }

    #[test]
    fn motorola_signed_scaled_and_exact_64_bit_values() {
        let dbc = dbc("BO_ 1 Test: 8 PC\n SG_ value : 7|16@0- (0.5,10) [-100|100] \"V\" PC");
        assert_eq!(
            &encode(&dbc, 1, false, &["9".into()]).unwrap()[..2],
            [0xff, 0xfe]
        );
        assert!(encode(&dbc, 1, false, &["9.1".into()]).is_err());
        let dbc = super::tests::dbc("BO_ 1 Test: 8 PC\n SG_ value : 0|64@1+ (1,0) [0|0] \"\" PC");
        assert_eq!(
            encode(&dbc, 1, false, &["0xffffffffffffffff".into()]).unwrap(),
            [0xff; 8]
        );
        assert!(encode(&dbc, 1, false, &["18446744073709551616".into()]).is_err());
    }

    #[test]
    fn rejects_malformed_and_unsupported_layouts() {
        for definition in [
            "BO_ 1 Bad: 1 PC\n SG_ bad : 7|8@1+ (1,0) [0|0] \"\" PC",
            "BO_ 1 Bad: 1 PC\n SG_ bad m1 : 0|8@1+ (1,0) [0|0] \"\" PC",
            "BO_ 1 Bad: 9 PC",
        ] {
            let dbc = dbc(definition);
            assert!(!definitions(&dbc)[0].error.is_empty());
            assert!(encode(&dbc, 1, false, &["0".into()]).is_err());
        }
        let dbc = dbc("BO_ 1 Request: 0 PC");
        assert!(encode(&dbc, 1, false, &[]).unwrap().is_empty());
    }
}
