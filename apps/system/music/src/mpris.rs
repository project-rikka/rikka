use std::time::{Duration, Instant};

use music_core::Repeat;
use rikka_app::prelude::*;
use rikka_mpris::{
    Abilities, Art, Command, Commands, Playback, Publisher, Repeat as Looping, Status, Track,
};

use crate::player::Player;

const PROGRAM: &str = "rikka-music";
const ART_WIDTH: u32 = 512;
const COMMANDS: &str = "mpris-commands";

pub(crate) fn publish() {
    let identity = tr!("music.title").to_string();
    spawn_blocking(
        move || {
            Publisher::builder(PROGRAM)
                .identity(identity)
                .desktop_entry(PROGRAM)
                .serve()
        },
        |player: &mut Player, served: rikka_mpris::Result<(Publisher, Commands)>| match served {
            Ok((publisher, commands)) => {
                player.publish_to(publisher);
                stream_keyed(COMMANDS, commands, handle_command);
            }
            Err(error) => tracing::warn!("failed to publish MPRIS player: {error}"),
        },
    );
}

pub(crate) fn playback(player: &Player) -> Playback {
    let Some(now) = player.now() else {
        return Playback::default();
    };
    let clock = player.clock();
    Playback {
        status: match player.playing() {
            true => Status::Playing,
            false => Status::Paused,
        },
        track: Some(Track {
            id: format!("{}:{}", now.service.slug, now.id.as_str()),
            title: now.name.clone(),
            artists: now
                .artists
                .iter()
                .map(|credit| credit.name.clone())
                .collect(),
            album: Some(now.album.name.clone()).filter(|album| !album.is_empty()),
            album_artists: Vec::new(),
            length: Some(clock.length()).filter(|length| !length.is_zero()),
            art: now
                .artwork
                .fitting(ART_WIDTH)
                .map(|url| Art::Web(url.to_owned())),
            url: None,
            number: Some(now.number).filter(|number| *number > 0),
        }),
        position: clock.position(Instant::now()),
        rate: 1.0,
        volume: f64::from(player.volume()),
        shuffle: Some(player.shuffle()),
        repeat: Some(match player.repeat() {
            Repeat::Off => Looping::None,
            Repeat::All => Looping::Playlist,
            Repeat::One => Looping::Track,
        }),
        abilities: Abilities {
            play: true,
            pause: true,
            next: player.has_next(),
            previous: true,
            seek: !clock.length().is_zero(),
            control: true,
        },
    }
}

fn handle_command(player: &mut Player, command: Command) {
    match command {
        Command::Play if !player.playing() => player.toggle(),
        Command::Pause if player.playing() => player.toggle(),
        Command::PlayPause => player.toggle(),
        Command::Stop => {
            if player.playing() {
                player.toggle();
            }
            player.seek(Duration::ZERO);
        }
        Command::Next => player.next(),
        Command::Previous => player.previous(),
        Command::SeekBy { forward, by } => {
            let at = player.clock().position(Instant::now());
            let to = match forward {
                true => at + by,
                false => at.saturating_sub(by),
            };
            match player.has_next() && to >= player.clock().length() && forward {
                true => player.next(),
                false => player.seek(to.min(player.clock().length())),
            }
        }
        Command::SeekTo(at) => player.seek(at),
        Command::Volume(volume) => player.set_volume(volume.clamp(0.0, 1.0) as f32),
        Command::Shuffle(shuffle) if shuffle != player.shuffle() => player.toggle_shuffle(),
        Command::Repeat(looping) => player.set_repeat(match looping {
            Looping::None => Repeat::Off,
            Looping::Playlist => Repeat::All,
            Looping::Track => Repeat::One,
        }),
        Command::Play | Command::Pause | Command::Shuffle(_) | Command::Raise | Command::Quit => {
            skip_view()
        }
    }
}
