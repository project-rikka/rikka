use desk_layout::Kind;
use rikka_ui::{Label, MaterialSymbol, tr};

#[derive(Debug)]
pub struct About {
    pub name: Label,
    pub symbol: MaterialSymbol,
}

#[must_use]
pub fn about(kind: Kind) -> About {
    let (label, symbol) = match kind {
        Kind::Clock => ("clock", MaterialSymbol::SCHEDULE),
        Kind::Calendar => ("calendar", MaterialSymbol::CALENDAR_MONTH),
        Kind::Weather => ("weather", MaterialSymbol::PARTLY_CLOUDY_DAY),
        Kind::Media => ("media", MaterialSymbol::MUSIC_NOTE),
        Kind::Monitor => ("monitor", MaterialSymbol::MONITORING),
        Kind::Power => ("power", MaterialSymbol::BATTERY_FULL),
        Kind::Note => ("note", MaterialSymbol::STICKY_NOTE_2),
        Kind::Photos => ("photos", MaterialSymbol::PHOTO_LIBRARY),
    };
    About {
        name: tr!(&format!("shell.desk.widget.{label}.name")),
        symbol,
    }
}
