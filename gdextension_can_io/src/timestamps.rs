//! Translate receive clocks by a fixed offset, preserving device-clock deltas.
//! Hardware clocks are backend-specific microsecond counters, not UNIX time.

const RESET_BACKSTEP_US: u64 = 1_000_000;

#[derive(Clone, Copy)]
struct Anchor {
    source_us: u64,
    target_us: u64,
    latest_us: u64,
}

#[derive(Default)]
pub struct Clock {
    anchor: Option<Anchor>,
}

impl Clock {
    /// Align the first source timestamp to the reference clock. Later samples
    /// use the same offset, regardless of queueing or receive-time jitter.
    /// Missing timestamps use the reference clock without changing the anchor.
    pub fn timestamp(&mut self, source_us: Option<u64>, reference_us: u64) -> u64 {
        let Some(source_us) = source_us else {
            return reference_us;
        };
        let anchor = self.anchor.get_or_insert(Anchor {
            source_us,
            target_us: reference_us,
            latest_us: source_us,
        });
        anchor.latest_us = anchor.latest_us.max(source_us);
        // Signed arithmetic preserves small out-of-order samples and handles
        // counters larger than the target clock without unsigned underflow.
        (anchor.target_us as i128 + source_us as i128 - anchor.source_us as i128)
            .clamp(0, u64::MAX as i128) as u64
    }

    /// Only an actual hardware counter can trigger device-reset handling.
    /// Host-only packets and wall-clock adjustments must not rebase hardware.
    pub fn hardware_timestamp(&mut self, source_us: Option<u64>, reference_us: u64) -> u64 {
        if let (Some(source_us), Some(anchor)) = (source_us, self.anchor) {
            if source_us.saturating_add(RESET_BACKSTEP_US) < anchor.latest_us {
                self.anchor = None;
            }
        }
        self.timestamp(source_us, reference_us)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_intervals_survive_host_and_processing_jitter() {
        let mut device = Clock::default();
        let mut application = Clock::default();
        let mut times = Vec::new();
        for (hardware, host, processed) in [
            (9_000_000, 1_700_000_000_000_000, 500_000),
            (9_001_000, 1_700_000_000_070_000, 600_000),
            (9_002_000, 1_700_000_000_090_000, 650_000),
        ] {
            let epoch = device.hardware_timestamp(Some(hardware), host);
            times.push(application.timestamp(Some(epoch), processed));
        }
        assert_eq!(times, [500_000, 501_000, 502_000]);
    }

    #[test]
    fn host_only_clock_jumps_do_not_rebase_the_hardware_timeline() {
        let mut device = Clock::default();
        let mut application = Clock::default();
        let epoch = device.hardware_timestamp(Some(1000), 1_700_000_000_000_000);
        assert_eq!(application.timestamp(Some(epoch), 500_000), 500_000);
        let fallback = device.hardware_timestamp(None, 1_700_000_010_000_000);
        application.timestamp(Some(fallback), 600_000);
        let epoch = device.hardware_timestamp(Some(2000), 1_699_999_990_000_000);
        assert_eq!(application.timestamp(Some(epoch), 900_000), 501_000);
    }

    #[test]
    fn missing_or_zero_hardware_timestamps_and_late_availability_are_supported() {
        let mut clock = Clock::default();
        assert_eq!(clock.timestamp(None, 100), 100);
        assert_eq!(clock.timestamp(Some(0), 200), 200);
        assert_eq!(clock.timestamp(None, 250), 250);
        assert_eq!(clock.timestamp(Some(100), 800), 300);
    }

    #[test]
    fn reordered_samples_wrap_extension_and_clock_resets_are_safe() {
        let mut clock = Clock::default();
        let start = u32::MAX as u64 - 100;
        assert_eq!(clock.timestamp(Some(start), 10_000), 10_000);
        assert_eq!(clock.timestamp(Some(start + 200), 20_000), 10_200);
        assert_eq!(clock.timestamp(Some(start + 190), 30_000), 10_190);
        assert_eq!(clock.timestamp(Some(start + 300), 40_000), 10_300);
        // The driver extends the 32-bit hardware counter. A device restart
        // instead makes a substantial backward jump and establishes a new offset.
        assert_eq!(clock.hardware_timestamp(Some(0), 50_000), 50_000);
        assert_eq!(clock.hardware_timestamp(Some(1000), 90_000), 51_000);
        let mut reconnected = Clock::default();
        assert_eq!(reconnected.timestamp(Some(500), 100_000), 100_000);
    }
}
