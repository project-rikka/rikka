use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use music_core::Quality;
use music_spotify_api::{AudioFormat, Date, Restriction, Track as TrackMessage};
use music_spotify_session::Account;
use music_spotify_wire::FileId;

const PREMIUM: &str = "premium";
const COUNTRY_LEN: usize = 2;
const FIRST_MONTH: i32 = 1;
const FIRST_DAY: i32 = 1;

pub(crate) struct Listener {
    country: Option<String>,
    catalogue: String,
    filters_explicit: bool,
}

impl Listener {
    pub(crate) fn of(account: &Account) -> Listener {
        Listener {
            catalogue: account
                .product
                .catalogue
                .clone()
                .unwrap_or_else(|| PREMIUM.to_owned()),
            country: account.country.clone(),
            filters_explicit: account.product.filters_explicit,
        }
    }

    pub(crate) fn file(&self, track: &TrackMessage, quality: Quality) -> Option<FileId> {
        if !self.may_hear(track) {
            return None;
        }
        if !track.file.is_empty() {
            return vorbis(track, quality);
        }
        track
            .alternative
            .iter()
            .filter(|alternative| self.may_hear(alternative))
            .find_map(|alternative| vorbis(alternative, quality))
    }

    fn may_hear(&self, track: &TrackMessage) -> bool {
        !(self.filters_explicit && track.explicit)
            && released(track)
            && self.licensed(&track.restriction)
    }

    fn licensed(&self, restrictions: &[Restriction]) -> bool {
        restrictions
            .iter()
            .filter(|restriction| restriction.catalogue_str.contains(&self.catalogue))
            .find_map(|restriction| {
                if let Some(allowed) = &restriction.countries_allowed {
                    Some(self.listed(allowed))
                } else {
                    restriction
                        .countries_forbidden
                        .as_ref()
                        .map(|forbidden| !self.listed(forbidden))
                }
            })
            .unwrap_or(true)
    }

    fn listed(&self, countries: &str) -> bool {
        self.country.as_deref().is_some_and(|country| {
            countries
                .as_bytes()
                .chunks(COUNTRY_LEN)
                .any(|listed| listed == country.as_bytes())
        })
    }
}

fn released(track: &TrackMessage) -> bool {
    let now = Timestamp::now();
    now.as_millisecond() >= track.earliest_live_timestamp
        && (track.availability.is_empty()
            || track.availability.iter().any(|availability| {
                starts(availability.start.as_ref()).is_some_and(|start| now >= start)
            }))
}

fn starts(date: Option<&Date>) -> Option<Timestamp> {
    let default = Date::default();
    let date = date.unwrap_or(&default);
    let civil = DateTime::new(
        i16::try_from(date.year).ok()?,
        i8::try_from(date.month.unwrap_or(FIRST_MONTH)).ok()?,
        i8::try_from(date.day.unwrap_or(FIRST_DAY)).ok()?,
        i8::try_from(date.hour).ok()?,
        i8::try_from(date.minute).ok()?,
        0,
        0,
    )
    .ok()?;
    Some(civil.to_zoned(TimeZone::UTC).ok()?.timestamp())
}

fn vorbis(track: &TrackMessage, quality: Quality) -> Option<FileId> {
    preference(quality).iter().find_map(|wanted| {
        track
            .file
            .iter()
            .filter(|file| {
                file.format
                    .and_then(|format| AudioFormat::try_from(format).ok())
                    == Some(*wanted)
            })
            .find_map(|file| FileId::from_bytes(&file.file_id).ok())
    })
}

fn preference(quality: Quality) -> [AudioFormat; 3] {
    match quality {
        Quality::Low => [
            AudioFormat::OggVorbis96,
            AudioFormat::OggVorbis160,
            AudioFormat::OggVorbis320,
        ],
        Quality::Standard => [
            AudioFormat::OggVorbis160,
            AudioFormat::OggVorbis96,
            AudioFormat::OggVorbis320,
        ],
        Quality::Highest => [
            AudioFormat::OggVorbis320,
            AudioFormat::OggVorbis160,
            AudioFormat::OggVorbis96,
        ],
    }
}
