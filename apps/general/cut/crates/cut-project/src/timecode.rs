use std::fmt;

use crate::rate::Rate;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timecode {
    negative: bool,
    hours: i64,
    minutes: i64,
    seconds: i64,
    frames: i64,
    dropped: bool,
}

impl Timecode {
    #[must_use]
    pub fn from_frame(frame: i64, rate: Rate) -> Timecode {
        let nominal = i64::from(rate.nominal());
        let dropped = rate.drops_frames();
        let counted = frame.unsigned_abs().min(i64::MAX as u64) as i64;
        let label = if dropped {
            drop_frame_label(counted, nominal)
        } else {
            counted
        };
        let per_hour = nominal * 3600;
        let per_minute = nominal * 60;
        Timecode {
            negative: frame < 0,
            hours: label / per_hour,
            minutes: label % per_hour / per_minute,
            seconds: label % per_minute / nominal,
            frames: label % nominal,
            dropped,
        }
    }
}

impl Timecode {
    #[must_use]
    pub fn parse(text: &str, rate: Rate) -> Option<i64> {
        let text = text.trim();
        let (negative, text) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let fields: Vec<i64> = text
            .split([':', ';', '.'])
            .map(|field| field.trim().parse::<i64>().ok().filter(|value| *value >= 0))
            .collect::<Option<_>>()?;
        if fields.is_empty() || fields.len() > 4 {
            return None;
        }
        let nominal = i64::from(rate.nominal());
        let frame = match fields.len() {
            1 => fields[0],
            _ => {
                let mut padded = [0; 4];
                padded[4 - fields.len()..].copy_from_slice(&fields);
                let [hours, minutes, seconds, frames] = padded;
                let label = ((hours * 60 + minutes) * 60 + seconds) * nominal + frames;
                match rate.drops_frames() {
                    true => {
                        let total_minutes = hours * 60 + minutes;
                        label - nominal / 15 * (total_minutes - total_minutes / 10)
                    }
                    false => label,
                }
            }
        };
        Some(if negative { -frame } else { frame })
    }
}

fn drop_frame_label(frame: i64, nominal: i64) -> i64 {
    let skipped = nominal / 15;
    let per_minute = nominal * 60 - skipped;
    let per_ten_minutes = nominal * 600 - skipped * 9;
    let tens = frame / per_ten_minutes;
    let within = frame % per_ten_minutes;
    let minutes = match within > skipped {
        true => (within - skipped) / per_minute,
        false => 0,
    };
    frame + skipped * 9 * tens + skipped * minutes
}

impl fmt::Display for Timecode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.negative { "-" } else { "" };
        let separator = if self.dropped { ';' } else { ':' };
        write!(
            f,
            "{sign}{:02}:{:02}:{:02}{separator}{:02}",
            self.hours, self.minutes, self.seconds, self.frames
        )
    }
}
