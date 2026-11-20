mod controls;
mod scrubber;

use std::time::Duration;

use desk_layout::{Media, Size};
use rikka_mpris::{Control, Player, Track};
use rikka_sound::Spectrum;
use rikka_ui::{
    Align, Element, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View, text, tr,
};
use rikka_widgets::{column, row, spacer};
use shell_feeds::art;
use shell_kit::Period;

use crate::widgets::card::{INNER, padded};
use crate::widgets::kit::{Header, LINE, header};
use crate::widgets::live::Live;
use crate::widgets::media::controls::{Deck, controls, play, remaining, times};
use crate::widgets::media::scrubber::scrubber;
use crate::widgets::quiet::quiet;
use crate::widgets::sources::Sources;

const SMALL_ART: f32 = 64.0;
const SMALL_DISC: f32 = 36.0;
const SMALL_SCRUB: f32 = 16.0;
const SMALL_BARS: usize = 18;
const MEDIUM_ART: f32 = 128.0;
const MEDIUM_DISC: f32 = 30.0;
const MEDIUM_SCRUB: f32 = 20.0;
const MEDIUM_BARS: usize = 24;
const LARGE_ART: f32 = 140.0;
const LARGE_DISC: f32 = 44.0;
const LARGE_SCRUB: f32 = 56.0;
const LARGE_BARS: usize = 44;

struct Playing<'a> {
    player: &'a Player,
    spectrum: Option<Spectrum>,
    length: Option<Duration>,
    at: Duration,
}

#[must_use]
pub(crate) fn view(media: &Media, size: Size, sources: &Sources) -> Element {
    let Some(player) = sources.player.get() else {
        return quiet(
            MaterialSymbol::MUSIC_NOTE,
            tr!("shell.desk.widget.media.idle"),
        )
        .sized(size);
    };
    let length = track(&player)
        .and_then(|track| track.length)
        .filter(|length| !length.is_zero());
    if player.is_playing() && length.is_some() {
        sources.feeds.wall.follow(Period::Second);
    }
    let playing = Playing {
        player: &player,
        spectrum: (media.waveform && player.is_playing())
            .then(|| sources.audio_capture.spectrum())
            .flatten(),
        length,
        at: length.map_or(Duration::ZERO, |length| player.position_now().min(length)),
    };
    match size {
        Size::Small => small(&playing),
        Size::Medium => medium(&playing),
        Size::Large | Size::ExtraLarge => large(&playing),
    }
}

impl Playing<'_> {
    fn scrub(&self, bars: usize, height: f32) -> Element {
        let fraction = self
            .length
            .map(|length| self.at.as_secs_f32() / length.as_secs_f32());
        let scrubbing = scrubber(self.spectrum.clone(), bars, fraction)
            .height(Length::px(height))
            .fill_width();
        match self.length.filter(|_| self.player.abilities.seek) {
            Some(length) => {
                let id = self.player.id.clone();
                scrubbing
                    .on_seek(move |live: &mut Live, to: f32| {
                        live.control(&id, Control::SeekTo(length.mul_f32(to)));
                    })
                    .build()
            }
            None => scrubbing.build(),
        }
    }

    fn media_header(&self) -> Header {
        header(
            Some(MaterialSymbol::GRAPHIC_EQ),
            Label::from(self.player.identity.as_str()),
        )
    }

    fn title(&self) -> Label {
        Label::from(
            track(self.player)
                .map(|track| track.title.clone())
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| self.player.identity.clone()),
        )
    }

    fn artist(&self) -> Option<Label> {
        track(self.player).and_then(Track::artist).map(Label::from)
    }

    fn album(&self) -> Option<Label> {
        track(self.player)
            .and_then(|track| track.album.clone())
            .filter(|album| !album.is_empty())
            .map(Label::from)
    }

    fn cover(&self, side: f32) -> Element {
        art(track(self.player), side, INNER)
    }
}

fn track(player: &Player) -> Option<&Track> {
    player.track.as_ref()
}

fn small(playing: &Playing<'_>) -> Element {
    padded(
        column! {
            row! {
                playing.cover(SMALL_ART);
                spacer().grow(1.0);
                play(playing.player, SMALL_DISC);
            }
            .align_items(Align::Start)
            .fill_width();
            spacer().grow(1.0);
            column! {
                text(playing.title()).label().semibold().no_wrap();
                if let Some(artist) = playing.artist() {
                    text(artist).caption().muted().no_wrap();
                }
            }
            .gap(LINE)
            .fill_width();
            playing.scrub(SMALL_BARS, SMALL_SCRUB);
        }
        .gap(Space::SNUG)
        .fill(),
    )
}

fn medium(playing: &Playing<'_>) -> Element {
    let media_header = match playing.length {
        Some(length) => playing
            .media_header()
            .trailing(remaining(playing.at, length)),
        None => playing.media_header(),
    };
    padded(
        row! {
            playing.cover(MEDIUM_ART);
            column! {
                media_header;
                column! {
                    text(playing.title()).semibold().no_wrap();
                    if let Some(artist) = playing.artist() {
                        text(artist).label().muted().no_wrap();
                    }
                }
                .gap(LINE)
                .fill_width();
                spacer().grow(1.0);
                playing.scrub(MEDIUM_BARS, MEDIUM_SCRUB);
                controls(playing.player, Deck::Compact, MEDIUM_DISC);
            }
            .gap(Space::TIGHT)
            .grow(1.0)
            .min_width(0.0)
            .fill_height();
        }
        .gap(Space::NORMAL)
        .fill(),
    )
}

fn large(playing: &Playing<'_>) -> Element {
    padded(
        column! {
            row! {
                playing.cover(LARGE_ART);
                column! {
                    playing.media_header();
                    spacer().grow(1.0);
                    text(playing.title()).subheading().semibold().no_wrap();
                    if let Some(artist) = playing.artist() {
                        text(artist).muted().no_wrap();
                    }
                    if let Some(album) = playing.album() {
                        text(album).label().muted().no_wrap();
                    }
                }
                .gap(LINE)
                .grow(1.0)
                .min_width(0.0)
                .height(Length::px(LARGE_ART));
            }
            .gap(Space::NORMAL)
            .fill_width();
            spacer().grow(1.0);
            column! {
                playing.scrub(LARGE_BARS, LARGE_SCRUB);
                if let Some(length) = playing.length {
                    times(playing.at, length);
                }
            }
            .gap(Space::TIGHT)
            .fill_width();
            spacer().grow(1.0);
            controls(playing.player, Deck::Full, LARGE_DISC);
        }
        .fill(),
    )
}
