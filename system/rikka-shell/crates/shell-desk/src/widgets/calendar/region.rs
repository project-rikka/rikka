use desk_layout::FirstDay;
use jiff::civil::Weekday;

const SUNDAY_FIRST: &[&str] = &[
    "AG", "AS", "BD", "BR", "BS", "BT", "BW", "BZ", "CA", "CO", "DM", "DO", "ET", "GT", "GU", "HK",
    "HN", "ID", "IL", "IN", "JM", "JP", "KE", "KH", "KR", "LA", "MH", "MM", "MO", "MT", "MX", "MZ",
    "NI", "NP", "PA", "PE", "PH", "PK", "PR", "PT", "PY", "SA", "SG", "SV", "TH", "TT", "TW", "UM",
    "US", "VE", "VI", "WS", "YE", "ZA", "ZW",
];
const SATURDAY_FIRST: &[&str] = &[
    "AE", "AF", "BH", "DJ", "DZ", "EG", "IQ", "IR", "JO", "KW", "LY", "OM", "QA", "SD", "SY",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Week {
    pub(crate) first: Weekday,
    weekend: [Weekday; 2],
}

impl Week {
    #[must_use]
    pub(super) fn from_first_day(choice: FirstDay) -> Week {
        let region = region();
        let region = region.as_deref().unwrap_or_default();
        let resting_early = SATURDAY_FIRST.contains(&region);
        let first = match choice {
            FirstDay::Saturday => Weekday::Saturday,
            FirstDay::Sunday => Weekday::Sunday,
            FirstDay::Monday => Weekday::Monday,
            FirstDay::Locale if resting_early => Weekday::Saturday,
            FirstDay::Locale if SUNDAY_FIRST.contains(&region) => Weekday::Sunday,
            FirstDay::Locale => Weekday::Monday,
        };
        let weekend = match resting_early {
            true => [Weekday::Friday, Weekday::Saturday],
            false => [Weekday::Saturday, Weekday::Sunday],
        };
        Week { first, weekend }
    }

    #[must_use]
    pub(crate) fn days(self) -> [Weekday; 7] {
        std::array::from_fn(|step| self.first.wrapping_add(step as i64))
    }

    #[must_use]
    pub(super) fn rests(self, day: Weekday) -> bool {
        self.weekend.contains(&day)
    }
}

fn region() -> Option<String> {
    rikka_types::LocaleCategory::Time
        .locale()
        .and_then(|locale| locale.territory().map(str::to_ascii_uppercase))
        .or_else(|| {
            rikka_i18n::current_locale()
                .region()
                .map(str::to_ascii_uppercase)
        })
}
