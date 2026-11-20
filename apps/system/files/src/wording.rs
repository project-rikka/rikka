use files_convert::{ModelFormat, MovieFormat, Scale, SoundFormat, Target};
use files_core::ViewMode;
use files_drives::Volume;
use files_places::{Group, Place, PlaceIcon};
use files_vfs::Error;
use rikka_app::prelude::*;
use rikka_file_operations::{Action, Kind, Phase, Progress, TaskKind};
use rikka_format::format_bytes;

pub(crate) fn view_mode(mode: ViewMode) -> Label {
    match mode {
        ViewMode::Icons => tr!("files.view.icons"),
        ViewMode::Details => tr!("files.view.details"),
    }
}

pub(crate) fn failure(error: &Error) -> String {
    match error {
        Error::NotFound { at } => tr!("files.error.not-found", name = &at.name()).to_string(),
        Error::Denied { at } => tr!("files.error.denied", name = &at.name()).to_string(),
        Error::Exists { at } => tr!("files.error.exists", name = &at.name()).to_string(),
        Error::WrongKind { at, expected } => {
            tr!(wrong_kind_key(*expected), name = &at.name()).to_string()
        }
        Error::NotEmpty { at } => tr!("files.error.not-empty", name = &at.name()).to_string(),
        Error::NoSpace { at } => tr!("files.error.no-space", name = &at.name()).to_string(),
        Error::IntoItself { at } => tr!("files.error.into-itself", name = &at.name()).to_string(),
        Error::BadName { name } if name.is_empty() => tr!("files.error.empty-name").to_string(),
        Error::BadName { name } => tr!("files.error.bad-name", name = name).to_string(),
        Error::NoBackend { at } => tr!("files.error.no-backend", scheme = at.scheme()).to_string(),
        Error::Unsupported { at, action } => {
            tr!(unsupported(*action), name = &at.name()).to_string()
        }
        Error::Locked { at, wrong: false } => {
            tr!("files.error.locked", name = &at.name()).to_string()
        }
        Error::Locked { at, wrong: true } => {
            tr!("files.error.wrong-password", name = &at.name()).to_string()
        }
        Error::Cancelled => tr!("files.error.cancelled").to_string(),
        Error::Other { at, message } => {
            tr!("files.error.other", name = &at.name(), message = message).to_string()
        }
        other => tr!("files.error.unknown", name = &other.to_string()).to_string(),
    }
}

fn wrong_kind_key(expected: Kind) -> &'static str {
    match expected {
        Kind::Folder => "files.error.wrong-kind.folder",
        Kind::File => "files.error.wrong-kind.file",
    }
}

fn unsupported(action: Action) -> &'static str {
    match action {
        Action::Anything => "files.error.unsupported.anything",
        Action::Create => "files.error.unsupported.create",
        Action::Rename => "files.error.unsupported.rename",
        Action::Trash => "files.error.unsupported.trash",
        Action::Write => "files.error.unsupported.write",
        Action::Move => "files.error.unsupported.move",
        Action::Restore => "files.error.unsupported.restore",
    }
}

pub(crate) fn new_name(folder: bool) -> String {
    tr!(if folder {
        "files.new.folder"
    } else {
        "files.new.file"
    })
    .to_string()
}

pub(crate) fn launch(error: &rikka_open::Error) -> String {
    tr!("files.error.launch", reason = &error.to_string()).to_string()
}

pub(crate) fn bookmarks(error: &std::io::Error) -> String {
    tr!("files.error.bookmarks", reason = &error.to_string()).to_string()
}

pub(crate) fn wallpaper(error: &files_wallpaper::Error) -> String {
    tr!("files.error.wallpaper", reason = &error.to_string()).to_string()
}

pub(crate) fn job(kind: TaskKind) -> String {
    tr!(match kind {
        TaskKind::Copy => "files.job.copy",
        TaskKind::Move => "files.job.move",
        TaskKind::Trash => "files.job.trash",
        TaskKind::Delete => "files.job.delete",
        TaskKind::Restore => "files.job.restore",
        TaskKind::Compress => "files.job.compress",
        TaskKind::Extract => "files.job.extract",
        TaskKind::Convert => "files.job.convert",
    })
    .to_string()
}

pub(crate) fn undo(kind: files_undo::Kind) -> Label {
    tr!(match kind {
        files_undo::Kind::Copy => "files.undo.copy",
        files_undo::Kind::Move => "files.undo.move",
        files_undo::Kind::Rename => "files.undo.rename",
        files_undo::Kind::Trash => "files.undo.trash",
        files_undo::Kind::Restore => "files.undo.restore",
        files_undo::Kind::NewFolder => "files.undo.new-folder",
        files_undo::Kind::NewFile => "files.undo.new-file",
    })
}

pub(crate) fn redo(kind: files_undo::Kind) -> Label {
    tr!(match kind {
        files_undo::Kind::Copy => "files.redo.copy",
        files_undo::Kind::Move => "files.redo.move",
        files_undo::Kind::Rename => "files.redo.rename",
        files_undo::Kind::Trash => "files.redo.trash",
        files_undo::Kind::Restore => "files.redo.restore",
        files_undo::Kind::NewFolder => "files.redo.new-folder",
        files_undo::Kind::NewFile => "files.redo.new-file",
    })
}

pub(crate) fn place(place: &Place) -> String {
    if let Some(volume) = &place.volume {
        return self::volume(volume);
    }
    match &place.name {
        Some(name) => name.clone(),
        None => tr!(special_place_key(place.icon)).to_string(),
    }
}

pub(crate) fn volume(volume: &Volume) -> String {
    if let Some(label) = &volume.label {
        return label.clone();
    }
    if volume.is_root {
        return tr!("files.places.system").to_string();
    }
    if let Some(drive) = &volume.drive {
        return drive.clone();
    }
    if volume.size > 0 {
        return tr!("files.places.volume", size = &format_bytes(volume.size)).to_string();
    }
    volume.device.to_string_lossy().into_owned()
}

fn special_place_key(icon: PlaceIcon) -> &'static str {
    match icon {
        PlaceIcon::Trash => "files.places.trash",
        _ => "files.places.home",
    }
}

pub(crate) fn group(group: Group) -> Option<String> {
    match group {
        Group::Personal => None,
        Group::Pinned => Some(tr!("files.places.pinned").to_string()),
        Group::Devices => Some(tr!("files.places.devices").to_string()),
    }
}

pub(crate) fn device(error: &files_drives::Error) -> String {
    use files_drives::Error as Refusal;
    match error {
        Refusal::Unavailable => tr!("files.device.unavailable").to_string(),
        Refusal::NotAuthorized => tr!("files.device.not-authorized").to_string(),
        Refusal::Busy => tr!("files.device.busy").to_string(),
        Refusal::NotSupported => tr!("files.device.not-supported").to_string(),
        Refusal::AlreadyMounted => tr!("files.device.already-mounted").to_string(),
        Refusal::NotMounted => tr!("files.device.not-mounted").to_string(),
        Refusal::WrongPassphrase => tr!("files.device.wrong-passphrase").to_string(),
        Refusal::Refused { message } => tr!("files.device.refused", message = message).to_string(),
    }
}

pub(crate) fn kind(described: Option<String>, extension: Option<String>) -> String {
    if let Some(described) = described {
        return described;
    }
    match extension {
        Some(extension) => tr!(
            "files.kind.extension",
            extension = &extension.to_uppercase()
        )
        .to_string(),
        None => tr!("files.kind.generic").to_string(),
    }
}

fn exact(count: u64) -> String {
    tr!(
        "files.size.bytes",
        count = count,
        grouped = &rikka_format::group_digits(&count.to_string()),
    )
    .to_string()
}

pub(crate) fn size_detailed(size: u64) -> String {
    tr!(
        "files.size.detailed",
        rounded = &format_bytes(size),
        exact = &self::exact(size),
    )
    .to_string()
}

pub(crate) fn catalogs() -> rikka_i18n::I18nBuilder {
    rikka_i18n::embedded!()
}

pub(crate) fn job_status(kind: TaskKind, progress: &Progress) -> String {
    let task = job(kind);
    match progress.phase {
        Phase::Preparing => tr!("files.job.preparing", task = &task),
        _ => tr!(
            "files.job.progress",
            task = &task,
            done = progress.items_done,
            total = progress.items_total,
        ),
    }
    .to_string()
}

pub(crate) fn conversion(target: Target) -> Label {
    match target {
        Target::Picture(format) => Label::from(format.name()),
        Target::Shrink(scale) => self::scale(scale),
        Target::StripMetadata => tr!("files.convert.strip"),
        Target::Sound(format) | Target::Extract(format) => Label::from(sound(format)),
        Target::Movie(format) => match format {
            MovieFormat::Mp4 => Label::from("MP4 (H.264)"),
            MovieFormat::WebM => Label::from("WebM (VP9)"),
            MovieFormat::Mkv => Label::from("MKV"),
            MovieFormat::Gif => tr!("files.convert.animation"),
        },
        Target::Model(format) => Label::from(match format {
            ModelFormat::Glb => "GLB",
            ModelFormat::Gltf => "glTF",
            ModelFormat::Fbx => "FBX",
            ModelFormat::Pmx => "PMX",
            ModelFormat::Vrm => "VRM 1.0",
        }),
    }
}

pub(crate) fn conversion_choice(target: Target) -> Label {
    match target {
        Target::Extract(format) => tr!("files.convert.sound-only", format = sound(format)),
        other => conversion(other),
    }
}

fn sound(format: SoundFormat) -> &'static str {
    match format {
        SoundFormat::Mp3 => "MP3",
        SoundFormat::Aac => "M4A (AAC)",
        SoundFormat::Opus => "Opus",
        SoundFormat::Vorbis => "Ogg Vorbis",
        SoundFormat::Flac => "FLAC",
        SoundFormat::Alac => "M4A (ALAC)",
        SoundFormat::Wav => "WAV",
    }
}

pub(crate) fn scale(scale: Scale) -> Label {
    match scale {
        Scale::Original => tr!("files.convert.size-original"),
        Scale::Half => tr!("files.convert.size-half"),
        Scale::LongestSide(side) => tr!("files.convert.size-side", size = side),
        Scale::ShortSide(lines) => tr!("files.convert.size-lines", size = lines),
    }
}

pub(crate) fn conversion_suffix(target: Target) -> String {
    match target {
        Target::Shrink(Scale::Half) => tr!("files.convert.suffix-half"),
        Target::Shrink(Scale::LongestSide(side) | Scale::ShortSide(side)) => {
            tr!("files.convert.suffix-side", size = side)
        }
        Target::StripMetadata => tr!("files.convert.suffix-clean"),
        _ => tr!("files.convert.suffix-converted"),
    }
    .to_string()
}
