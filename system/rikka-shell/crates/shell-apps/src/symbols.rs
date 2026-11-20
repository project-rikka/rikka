use rikka_ui::MaterialSymbol;

const BY_CATEGORY: &[(&str, MaterialSymbol)] = &[
    ("TerminalEmulator", MaterialSymbol::TERMINAL),
    ("WebBrowser", MaterialSymbol::PUBLIC),
    ("Email", MaterialSymbol::MAIL),
    ("InstantMessaging", MaterialSymbol::CHAT),
    ("IRCClient", MaterialSymbol::CHAT),
    ("Chat", MaterialSymbol::CHAT),
    ("FileManager", MaterialSymbol::FOLDER),
    ("TextEditor", MaterialSymbol::EDIT_NOTE),
    ("IDE", MaterialSymbol::BUILD),
    ("Debugger", MaterialSymbol::BUILD),
    ("Calculator", MaterialSymbol::CALCULATE),
    ("Spreadsheet", MaterialSymbol::TABLE_CHART),
    ("WordProcessor", MaterialSymbol::DESCRIPTION),
    ("Presentation", MaterialSymbol::SLIDESHOW),
    ("Photography", MaterialSymbol::PHOTO_CAMERA),
    ("RasterGraphics", MaterialSymbol::BRUSH),
    ("VectorGraphics", MaterialSymbol::DRAW),
    ("Viewer", MaterialSymbol::VISIBILITY),
    ("Player", MaterialSymbol::MOVIE),
    ("Music", MaterialSymbol::MUSIC_NOTE),
    ("AudioVideoEditing", MaterialSymbol::VIDEOCAM),
    ("Monitor", MaterialSymbol::MONITOR_HEART),
    ("Security", MaterialSymbol::SHIELD),
    ("Archiving", MaterialSymbol::FOLDER_ZIP),
    ("Compression", MaterialSymbol::FOLDER_ZIP),
    ("Dictionary", MaterialSymbol::LANGUAGE),
    ("Settings", MaterialSymbol::TUNE),
    ("Audio", MaterialSymbol::HEADPHONES),
    ("Video", MaterialSymbol::MOVIE),
    ("AudioVideo", MaterialSymbol::MOVIE),
    ("Development", MaterialSymbol::BUILD),
    ("Education", MaterialSymbol::SCHOOL),
    ("Game", MaterialSymbol::SPORTS_ESPORTS),
    ("Graphics", MaterialSymbol::BRUSH),
    ("Network", MaterialSymbol::PUBLIC),
    ("Office", MaterialSymbol::DESCRIPTION),
    ("Science", MaterialSymbol::SCIENCE),
    ("System", MaterialSymbol::MEMORY),
    ("Utility", MaterialSymbol::WIDGETS),
];

pub(super) fn for_categories<S: AsRef<str>>(categories: &[S]) -> MaterialSymbol {
    BY_CATEGORY
        .iter()
        .find(|(name, _)| categories.iter().any(|declared| declared.as_ref() == *name))
        .map_or(MaterialSymbol::APPS, |(_, symbol)| *symbol)
}
