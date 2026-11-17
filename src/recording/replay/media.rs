use rtsp_types::Data;
use time::OffsetDateTime;

use crate::rtsp::rtp_extension;

use super::{RecordingGap, ReplayAccessUnit, ReplayErrorKind, ReplayPacket};

const REPLAY_EXTENSION: u16 = 0xABAC;
const NTP_UNIX_OFFSET: i64 = 2_208_988_800;
const ERA_SECONDS: i64 = 1 << 32;

#[derive(Clone, Copy)]
struct Extension {
    seconds: u32,
    fraction: u32,
    flags: u8,
    cseq: u8,
}

impl Extension {
    fn parse(bytes: &[u8]) -> Result<Self, ReplayErrorKind> {
        let [a, b, c, d, e, f, g, h, flags, cseq, _, _] = *bytes else {
            return Err(ReplayErrorKind::InvalidResponse);
        };
        if flags & 0x0f != 0 {
            return Err(ReplayErrorKind::InvalidResponse);
        }
        Ok(Self {
            seconds: u32::from_be_bytes([a, b, c, d]),
            fraction: u32::from_be_bytes([e, f, g, h]),
            flags,
            cseq,
        })
    }

    fn evidence(self, reference: OffsetDateTime) -> Result<ReplayAccessUnit, ReplayErrorKind> {
        let reference_seconds = reference.unix_timestamp() + NTP_UNIX_OFFSET;
        let offset_in_era = (reference_seconds - i64::from(self.seconds)).rem_euclid(ERA_SECONDS);
        let nanos = (u64::from(self.fraction) * 1_000_000_000) >> 32;
        let distance = i128::from(offset_in_era) * 1_000_000_000
            + i128::from(reference.nanosecond())
            - i128::from(nanos);
        let half_era = i128::from(ERA_SECONDS / 2) * 1_000_000_000;
        if distance.abs() == half_era {
            return Err(ReplayErrorKind::PositionUnconfirmed);
        }
        let mut seconds = reference_seconds - offset_in_era - NTP_UNIX_OFFSET;
        if distance > half_era {
            seconds += ERA_SECONDS;
        }
        let recorded_at = OffsetDateTime::from_unix_timestamp_nanos(
            i128::from(seconds) * 1_000_000_000 + i128::from(nanos),
        )
        .map_err(|_| ReplayErrorKind::PositionUnconfirmed)?;
        Ok(ReplayAccessUnit {
            recorded_at,
            clean_point: self.flags & 0x80 != 0,
            section_end: self.flags & 0x40 != 0,
            discontinuity: self.flags & 0x20 != 0,
            terminal: self.flags & 0x10 != 0,
        })
    }
}

pub(super) fn starts_position(data: &Data<Vec<u8>>, payload: u8, cseq: u32, seek: bool) -> bool {
    let Some(Some((REPLAY_EXTENSION, bytes))) = rtp_extension(data.as_slice(), payload) else {
        return false;
    };
    let Ok(extension) = Extension::parse(bytes) else {
        return false;
    };
    extension.cseq == cseq as u8 && (!seek || extension.flags & 0x20 != 0)
}

#[derive(Clone, Copy)]
struct Header {
    sequence: u16,
    timestamp: u32,
    ssrc: u32,
    marker: bool,
}

impl Header {
    fn follows(self, previous: Self) -> bool {
        self.ssrc == previous.ssrc && self.sequence == previous.sequence.wrapping_add(1)
    }
}

struct Fragment {
    header: Header,
    start: ReplayAccessUnit,
}

/// Correlation and continuity are local to one accepted PLAY.
pub(super) struct Position {
    cseq: u8,
    seek: bool,
    reference: OffsetDateTime,
    first: Option<ReplayAccessUnit>,
    previous: Option<Header>,
    fragment: Option<Fragment>,
    section_end: Option<OffsetDateTime>,
    terminal: bool,
}

impl Position {
    pub(super) fn new(cseq: u32, seek: bool, reference: OffsetDateTime) -> Self {
        Self {
            cseq: cseq.to_le_bytes()[0],
            seek,
            reference,
            first: None,
            previous: None,
            fragment: None,
            section_end: None,
            terminal: false,
        }
    }

    pub(super) fn first(&self) -> Option<ReplayAccessUnit> {
        self.first
    }

    pub(super) fn terminal(&self) -> bool {
        self.terminal
    }

    pub(super) fn accept(
        &mut self,
        data: Data<Vec<u8>>,
        payload: u8,
    ) -> Result<Option<ReplayPacket>, ReplayErrorKind> {
        if data.channel_id() != 0 {
            return Ok(None);
        }
        let bytes = data.into_body();
        let Some(extension) = rtp_extension(&bytes, payload) else {
            return Ok(None);
        };
        let header = Header {
            sequence: u16::from_be_bytes([bytes[2], bytes[3]]),
            timestamp: u32::from_be_bytes(bytes[4..8].try_into().expect("valid RTP header")),
            ssrc: u32::from_be_bytes(bytes[8..12].try_into().expect("valid RTP header")),
            marker: bytes[1] & 0x80 != 0,
        };
        let extension = match extension {
            Some((REPLAY_EXTENSION, bytes)) => Some(Extension::parse(bytes)?),
            _ => None,
        };
        if let Some(extension) = extension {
            if extension.cseq != self.cseq
                || (self.first.is_none() && self.seek && extension.flags & 0x20 == 0)
            {
                return Ok(None);
            }
            let unit = extension.evidence(self.reference)?;
            self.first.get_or_insert(unit);
            self.reference = unit.recorded_at;
            let discontinuity = self.fragment.take().is_some()
                || self.previous.is_some_and(|last| !header.follows(last));
            let gap_before = self.section_end.take().and_then(|end| {
                (!discontinuity && unit.clean_point && end < unit.recorded_at).then_some(
                    RecordingGap {
                        previous_section_ended_at: end,
                        next_section_started_at: unit.recorded_at,
                    },
                )
            });
            self.previous = Some(header);
            self.terminal = false;
            self.finish_or_retain(header, unit);
            return Ok(Some(ReplayPacket {
                bytes,
                access_unit: Some(unit),
                gap_before,
                transport_discontinuity: discontinuity,
            }));
        }

        if let Some(fragment) = self.fragment.take()
            && header.follows(fragment.header)
            && header.timestamp == fragment.header.timestamp
        {
            self.previous = Some(header);
            self.finish_or_retain(header, fragment.start);
            return Ok(Some(ReplayPacket {
                bytes,
                access_unit: None,
                gap_before: None,
                transport_discontinuity: false,
            }));
        }
        // We never attach an uncorrelated access unit to the requested
        // position. This packet also breaks any pending evidence of a gap.
        self.section_end = None;
        Ok(None)
    }

    fn finish_or_retain(&mut self, header: Header, unit: ReplayAccessUnit) {
        if header.marker {
            self.section_end = unit.section_end.then_some(unit.recorded_at);
            self.terminal = unit.terminal;
        } else {
            self.fragment = Some(Fragment {
                header,
                start: unit,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_replay_extension_is_rejected() {
        let mut bytes = vec![0x90, 96, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0xab, 0xac, 0, 2];
        bytes.extend([0; 8]);
        bytes.push(0x65);
        let mut position = Position::new(4, false, OffsetDateTime::UNIX_EPOCH);
        assert_eq!(
            position.accept(Data::new(0, bytes), 96).unwrap_err(),
            ReplayErrorKind::InvalidResponse
        );
        assert!(position.first().is_none());
    }

    #[test]
    fn ntp_era_uses_recording_context_and_preserves_fraction() {
        let rollover = OffsetDateTime::from_unix_timestamp(ERA_SECONDS - NTP_UNIX_OFFSET).unwrap();
        let ntp = Extension {
            seconds: 0,
            fraction: 0x8000_0000,
            flags: 0,
            cseq: 0,
        };
        assert_eq!(
            ntp.evidence(rollover)
                .unwrap()
                .recorded_at
                .unix_timestamp_nanos(),
            rollover.unix_timestamp_nanos() + 500_000_000,
        );
        let previous = Extension {
            seconds: u32::MAX,
            fraction: 0,
            ..ntp
        };
        assert_eq!(
            previous
                .evidence(rollover)
                .unwrap()
                .recorded_at
                .unix_timestamp(),
            rollover.unix_timestamp() - 1,
        );
        let reference =
            OffsetDateTime::from_unix_timestamp(ERA_SECONDS / 2 - NTP_UNIX_OFFSET).unwrap();
        let ambiguous = Extension { fraction: 0, ..ntp };
        assert_eq!(
            ambiguous.evidence(reference),
            Err(ReplayErrorKind::PositionUnconfirmed)
        );
    }
}
