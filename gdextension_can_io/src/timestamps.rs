//! Translate receive clocks by a fixed offset, preserving device-clock deltas.
//! Hardware clocks are backend-specific microsecond counters, not UNIX time.

/// Conversion from the canonical app timeline to a user-selected time origin.
/// The MF4 header epoch plus its channel offset always gives the same UTC time.
#[derive(Clone, Copy, Default)]
pub struct Origin {
    pub epoch_ns: u64,
    pub offset_us: i128,
}

impl Origin {
    pub fn timestamp_us(self, app_us: u64) -> i128 {
        i128::from(app_us) + self.offset_us
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Epoch {
    System = 0,
    #[default]
    Application = 1,
    Recording = 2,
}

impl TryFrom<i32> for Epoch {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::System),
            1 => Ok(Self::Application),
            2 => Ok(Self::Recording),
            _ => Err("Select system epoch, app time or restart on recording."),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Timeline {
    pub epoch: Epoch,
    app_epoch_ns: u64,
    recording_start_us: u64,
}

impl Timeline {
    pub fn new(app_epoch_ns: u64) -> Self {
        Self {
            epoch: Epoch::Application,
            app_epoch_ns: app_epoch_ns / 1000 * 1000,
            recording_start_us: 0,
        }
    }

    pub fn start_recording(&mut self, app_us: u64) {
        self.recording_start_us = app_us;
    }

    pub fn origin(self) -> Origin {
        match self.epoch {
            Epoch::System => Origin {
                epoch_ns: 0,
                offset_us: i128::from(self.app_epoch_ns / 1000),
            },
            Epoch::Application => Origin {
                epoch_ns: self.app_epoch_ns,
                offset_us: 0,
            },
            Epoch::Recording => Origin {
                epoch_ns: self
                    .app_epoch_ns
                    .saturating_add(self.recording_start_us.saturating_mul(1000)),
                offset_us: -i128::from(self.recording_start_us),
            },
        }
    }
}

pub fn format_seconds(us: i128) -> String {
    let sign = if us < 0 { "-" } else { "" };
    let us = us.unsigned_abs();
    format!("{sign}{:04}.{:06}", us / 1_000_000, us % 1_000_000)
}

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
    fn epoch_selection_preserves_utc_and_hardware_spacing() {
        let mut timeline = Timeline::new(1_710_000_000_123_456_789);
        timeline.start_recording(3_600_000_000);
        for mode in 0..=2 {
            timeline.epoch = Epoch::try_from(mode).unwrap();
            let origin = timeline.origin();
            let stamp = origin.timestamp_us(3_600_001_000);
            assert_eq!(
                i128::from(origin.epoch_ns) + stamp * 1000,
                1_710_003_600_124_456_000
            );
            assert_eq!(origin.timestamp_us(3_600_002_000) - stamp, 1000);
        }
        assert!(Epoch::try_from(3).is_err());
        assert!(Epoch::try_from(-1).is_err());
    }

    #[test]
    fn recording_origin_restarts_without_clipping_queued_samples() {
        let mut timeline = Timeline::new(1_710_000_000_000_000_000);
        timeline.epoch = Epoch::Recording;
        assert_eq!(timeline.origin().timestamp_us(100), 100); // Before first recording.
        timeline.start_recording(5000);
        assert_eq!(timeline.origin().timestamp_us(4000), -1000);
        assert_eq!(format_seconds(-1000), "-0000.001000");
        timeline.start_recording(9000);
        assert_eq!(timeline.origin().timestamp_us(9000), 0);
        assert_eq!(format_seconds(1_710_000_000_123_456), "1710000000.123456");
    }

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
