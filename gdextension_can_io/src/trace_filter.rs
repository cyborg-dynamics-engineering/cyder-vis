//! candump-compatible filters, applied to trace and recording rather than the receive table.
use crosscan::can::CanFrame;

const EFF: u32 = 0x8000_0000;
const RTR: u32 = 0x4000_0000;
const ERR: u32 = 0x2000_0000;

#[derive(Clone, Debug, Default)]
pub struct TraceFilter {
    interface: Option<String>,
    rules: Vec<Rule>,
    join: bool,
    errors: u32,
}

#[derive(Clone, Debug)]
struct Rule {
    id: u32,
    mask: u32,
    inverted: bool,
}

impl TraceFilter {
    pub fn parse(input: &str) -> Result<Self, String> {
        let mut filter = Self::default();
        if input.trim().is_empty() {
            return Ok(filter);
        }
        for (index, token) in input.trim().split(',').map(str::trim).enumerate() {
            if token == "j" || token == "J" {
                filter.join = true;
            } else if let Some(mask) = token.strip_prefix('#') {
                filter.errors = hex(mask)? & 0x1fff_ffff;
            } else if let Some((id, mask)) = token.split_once(':').or_else(|| token.split_once('~'))
            {
                let id_value = hex(id)?;
                filter.rules.push(Rule {
                    id: id_value | if id.len() == 8 { EFF } else { 0 },
                    mask: hex(mask)? & !ERR,
                    inverted: token.contains('~') || id_value & ERR != 0,
                });
            } else if index == 0
                && !token.is_empty()
                && token.len() <= 15
                && token
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
            {
                filter.interface = Some(token.to_owned());
            } else {
                return Err(format!(
                    "Invalid filter '{token}'. Use ID:MASK, ID~MASK, #ERROR_MASK or j."
                ));
            }
        }
        Ok(filter)
    }

    pub fn matches(&self, frame: &CanFrame, interface: &str) -> bool {
        if let Some(expected) = &self.interface {
            if expected != "any" && expected != interface {
                return false;
            }
        }
        if frame.is_error() {
            return frame.id() & self.errors != 0;
        }
        let id = frame.id()
            | if frame.is_extended() { EFF } else { 0 }
            | if frame.is_rtr() { RTR } else { 0 };
        let matches = |rule: &Rule| ((id & rule.mask) == (rule.id & rule.mask)) != rule.inverted;
        self.rules.is_empty()
            || if self.join {
                self.rules.iter().all(matches)
            } else {
                self.rules.iter().any(matches)
            }
    }
}

fn hex(input: &str) -> Result<u32, String> {
    if input.is_empty() || input.len() > 8 || !input.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("Invalid hexadecimal ID or mask: '{input}'"));
    }
    u32::from_str_radix(input, 16).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(filter: &str, frame: CanFrame) -> bool {
        TraceFilter::parse(filter).unwrap().matches(&frame, "can0")
    }

    #[test]
    fn masks_or_inversion_and_join() {
        assert!(!matches("20000123:7FF", CanFrame::new(0x123, &[]).unwrap()));
        assert!(matches("", CanFrame::new(0x123, &[]).unwrap()));
        assert!(matches(
            "123:7FF,400:700",
            CanFrame::new(0x456, &[]).unwrap()
        ));
        assert!(!matches(
            "123:7FF,400:700",
            CanFrame::new(0x223, &[]).unwrap()
        ));
        assert!(!matches("400~700", CanFrame::new(0x456, &[]).unwrap()));
        assert!(matches(
            "j,400:700,450~7F0",
            CanFrame::new(0x460, &[]).unwrap()
        ));
        assert!(!matches(
            "400:700,450~7F0,J",
            CanFrame::new(0x456, &[]).unwrap()
        ));
        assert!(!matches("0~0", CanFrame::new(0x123, &[]).unwrap()));
    }

    #[test]
    fn extended_and_remote_flags() {
        assert!(matches("123:7FF", CanFrame::new_eff(0x123, &[]).unwrap()));
        assert!(matches(
            "123:7FF",
            CanFrame::new_remote(0x123, 0, false).unwrap()
        ));
        assert!(matches("123:C00007FF", CanFrame::new(0x123, &[]).unwrap()));
        assert!(!matches(
            "123:C00007FF",
            CanFrame::new_eff(0x123, &[]).unwrap()
        ));
        assert!(!matches(
            "123:C00007FF",
            CanFrame::new_remote(0x123, 0, false).unwrap()
        ));
        assert!(matches(
            "00000123:DFFFFFFF",
            CanFrame::new_eff(0x123, &[]).unwrap()
        ));
        assert!(!matches(
            "00000123:DFFFFFFF",
            CanFrame::new(0x123, &[]).unwrap()
        ));
    }

    #[test]
    fn errors_and_interface_prefix() {
        assert!(!matches("", CanFrame::new_error(4).unwrap()));
        assert!(matches(
            "any,0~0,#00000004",
            CanFrame::new_error(4).unwrap()
        ));
        assert!(!matches("0:0,#00000002", CanFrame::new_error(4).unwrap()));
        assert!(matches("can0,123:7FF", CanFrame::new(0x123, &[]).unwrap()));
        assert!(!matches("can1,123:7FF", CanFrame::new(0x123, &[]).unwrap()));
    }

    #[test]
    fn rejects_malformed_filters() {
        for input in [
            "123:",
            "123:xyz",
            "123:7ff,",
            ":7ff",
            "0:100000000",
            "0::0",
            "#",
            "j,bad",
        ] {
            assert!(TraceFilter::parse(input).is_err(), "{input}");
        }
    }
}
