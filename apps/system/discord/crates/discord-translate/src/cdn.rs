use discord_core::{Picture, StickerFormat};

const CDN: &str = "https://cdn.discordapp.com";
const MEDIA: &str = "https://media.discordapp.net";
const SIZE: &str = "size";
const PREVIEW_WIDTH: u32 = 1100;
const PREVIEW_HEIGHT: u32 = 700;
const LEGACY_AVATARS: u64 = 5;
const AVATARS: u64 = 6;

pub fn avatar(user: &str, hash: Option<&str>, discriminator: Option<&str>) -> Picture {
    match hash {
        Some(hash) => hashed(format!("{CDN}/avatars/{user}"), hash),
        None => default_avatar(user, discriminator),
    }
}

fn default_avatar(user: &str, discriminator: Option<&str>) -> Picture {
    let legacy = discriminator
        .and_then(|tag| tag.parse::<u64>().ok())
        .filter(|&tag| tag != 0);
    let index = match legacy {
        Some(tag) => tag % LEGACY_AVATARS,
        None => (user.parse::<u64>().unwrap_or(0) >> 22) % AVATARS,
    };
    Picture::fixed(format!("{CDN}/embed/avatars/{index}.png"))
}

pub(crate) fn member_avatar(guild: &str, user: &str, hash: &str) -> Picture {
    hashed(format!("{CDN}/guilds/{guild}/users/{user}/avatars"), hash)
}

pub(crate) fn guild_icon(guild: &str, hash: &str) -> Picture {
    hashed(format!("{CDN}/icons/{guild}"), hash)
}

pub(crate) fn guild_banner(guild: &str, hash: &str) -> Picture {
    hashed(format!("{CDN}/banners/{guild}"), hash)
}

pub(crate) fn user_banner(user: &str, hash: &str) -> Picture {
    hashed(format!("{CDN}/banners/{user}"), hash)
}

pub(crate) fn member_banner(guild: &str, user: &str, hash: &str) -> Picture {
    hashed(format!("{CDN}/guilds/{guild}/users/{user}/banners"), hash)
}

pub(crate) fn badge(icon: &str) -> Picture {
    Picture::fixed(format!("{CDN}/badge-icons/{icon}.png"))
}

pub(crate) fn activity_art(application: Option<&str>, asset: &str) -> Option<Picture> {
    if let Some(cover) = asset.strip_prefix("spotify:") {
        return Some(Picture::fixed(format!("https://i.scdn.co/image/{cover}")));
    }
    if let Some(proxied) = asset.strip_prefix("mp:") {
        return Some(Picture::fixed(format!("{MEDIA}/{proxied}")));
    }
    if let Some(video) = asset.strip_prefix("youtube:") {
        return Some(Picture::fixed(format!(
            "https://i.ytimg.com/vi/{video}/hqdefault_live.jpg"
        )));
    }
    if let Some(channel) = asset.strip_prefix("twitch:") {
        return Some(Picture::fixed(format!(
            "https://static-cdn.jtvnw.net/previews-ttv/live_user_{channel}-320x180.jpg"
        )));
    }
    let application = application?;
    asset
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| Picture::sized(format!("{CDN}/app-assets/{application}/{asset}.png"), SIZE))
}

pub(crate) fn role_icon(role: &str, hash: &str) -> Picture {
    Picture::sized(format!("{CDN}/role-icons/{role}/{hash}.png"), SIZE)
}

pub(crate) fn channel_icon(channel: &str, hash: &str) -> Picture {
    Picture::sized(format!("{CDN}/channel-icons/{channel}/{hash}.png"), SIZE)
}

pub(crate) fn sticker(id: &str, format: StickerFormat) -> Picture {
    match format {
        StickerFormat::Png => Picture::sized(format!("{MEDIA}/stickers/{id}.png"), SIZE),
        StickerFormat::Apng => {
            Picture::sized(format!("{MEDIA}/stickers/{id}.png"), SIZE).animated(true)
        }
        StickerFormat::Gif => {
            Picture::sized(format!("{MEDIA}/stickers/{id}.gif"), SIZE).animated(true)
        }
        StickerFormat::Lottie => {
            Picture::fixed(format!("https://discord.com/stickers/{id}.json")).animated(true)
        }
    }
}

pub(crate) fn preview(proxy: &str, width: u32, height: u32, video: bool) -> Picture {
    let (width, height) = fitted(width, height);
    let joint = if proxy.contains('?') { '&' } else { '?' };
    let format = if video { "format=webp&" } else { "" };
    Picture::fixed(format!(
        "{proxy}{joint}{format}width={width}&height={height}"
    ))
}

fn fitted(width: u32, height: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (width, height);
    }
    let scale = (f64::from(PREVIEW_WIDTH) / f64::from(width))
        .min(f64::from(PREVIEW_HEIGHT) / f64::from(height))
        .min(1.0);
    let scaled = |side: u32| ((f64::from(side) * scale).round() as u32).max(1);
    (scaled(width), scaled(height))
}

fn hashed(base: String, hash: &str) -> Picture {
    let animated = hash.starts_with("a_");
    let extension = if animated { "gif" } else { "png" };
    Picture::sized(format!("{base}/{hash}.{extension}"), SIZE).animated(animated)
}
