mod badge;
mod canvas;
mod figure;
mod movie;
mod music;
mod page;
mod picture;
mod player;
mod sound;
mod status;

use std::time::Duration;

use rikka_app::prelude::*;
use rikka_ui::search::Action;
use viewer_media::Kind;

use crate::open_file::OpenFile;
use crate::screen::figure::Figure;
use crate::screen::movie::Movie;
use crate::screen::music::Music;
use crate::screen::page::Page;
use crate::screen::picture::Picture;
use crate::screen::player::{Player, Ticker};
use crate::screen::status::StatusView;

#[derive(Debug, State)]
pub(crate) enum Screen {
    Picture(Picture),
    Movie(Movie),
    Music(Music),
    Figure(Figure),
    Page(Page),
    Status(StatusView),
}

impl Screen {
    pub(crate) fn open(file: &OpenFile) -> Self {
        let path = file.path();
        if let Some(reason) = file.error() {
            return Self::Status(StatusView::cannot_read(reason));
        }
        let Some(media) = file.media() else {
            return Self::Status(StatusView::unsupported());
        };
        match media.kind() {
            Kind::Picture if media.format.is_photoshop() => Self::Picture(Picture::flatten(path)),
            Kind::Picture => Self::Picture(Picture::open(path, media.extent)),
            Kind::Movie => Self::Movie(Movie::open(path)),
            Kind::Sound => Self::Music(Music::open(path)),
            Kind::Model => Self::Figure(Figure::open(path, media.format.is_motion())),
            Kind::Text => Self::Page(Page::open(path, media.format.is_markdown())),
        }
    }

    pub(crate) fn empty() -> Self {
        Self::Status(StatusView::empty())
    }

    pub(crate) fn start(&mut self) {
        Ticker::stop_all();
        match self {
            Self::Picture(picture) => picture.start(),
            Self::Movie(movie) => movie.start(),
            Self::Music(music) => music.start(),
            Self::Figure(figure) => figure.start(),
            Self::Page(page) => page.start(),
            Self::Status(_) => {}
        }
    }

    pub(crate) fn play_pause(&mut self) {
        if let Some(player) = self.player() {
            player.play_pause();
        }
    }

    pub(crate) fn seek(&mut self, seconds: f32) {
        if let Some(player) = self.player() {
            player.seek(Duration::from_secs_f32(seconds.max(0.0)));
        }
    }

    pub(crate) fn skip(&mut self, seconds: f32) {
        if let Some(player) = self.player() {
            player.skip(seconds);
        }
    }

    pub(crate) fn set_volume(&mut self, volume: f32) {
        if let Some(player) = self.player() {
            player.set_volume(volume);
        }
    }

    pub(crate) fn nudge_volume(&mut self, step: f32) {
        if let Some(player) = self.player() {
            player.nudge_volume(step);
        }
    }

    pub(crate) fn on_tick(&mut self) {
        if let Some(player) = self.player() {
            player.on_tick();
        }
    }

    pub(crate) fn save(&mut self) {
        if let Some(page) = self.page() {
            page.save();
        }
    }

    pub(crate) fn open_find(&mut self, replacing: bool) {
        if let Some(page) = self.page() {
            page.open_find(replacing);
        }
    }

    pub(crate) fn find(&mut self, action: Action) {
        if let Some(page) = self.page() {
            page.find(action);
        }
    }

    pub(crate) fn stage(&self) -> Element {
        match self {
            Self::Picture(picture) => picture.stage(),
            Self::Movie(movie) => movie.stage(),
            Self::Music(music) => music.stage(),
            Self::Figure(figure) => figure.stage(),
            Self::Page(page) => page.stage(),
            Self::Status(error) => error.stage(),
        }
    }

    pub(crate) fn controls(&self) -> Option<Element> {
        Some(self.playing()?.controls())
    }

    pub(crate) fn is_finding(&self) -> bool {
        match self {
            Self::Page(page) => page.is_finding(),
            _ => false,
        }
    }

    pub(crate) fn unsaved(&self) -> bool {
        match self {
            Self::Page(page) => page.unsaved(),
            _ => false,
        }
    }

    pub(crate) fn detail(&self) -> Option<Element> {
        match self {
            Self::Picture(picture) => picture.detail(),
            Self::Figure(figure) => figure.detail(),
            Self::Page(page) => page.detail(),
            _ => self.playing()?.detail(),
        }
    }

    fn picture(&mut self) -> Option<&mut Picture> {
        match self {
            Self::Picture(picture) => Some(picture),
            _ => None,
        }
    }

    fn page(&mut self) -> Option<&mut Page> {
        match self {
            Self::Page(page) => Some(page),
            _ => None,
        }
    }

    fn figure(&mut self) -> Option<&mut Figure> {
        match self {
            Self::Figure(figure) => Some(figure),
            _ => None,
        }
    }

    fn music(&mut self) -> Option<&mut Music> {
        match self {
            Self::Music(music) => Some(music),
            _ => None,
        }
    }

    fn player(&mut self) -> Option<&mut dyn Player> {
        match self {
            Self::Movie(movie) => Some(movie),
            Self::Music(music) => Some(music),
            Self::Figure(figure) if figure.animated() => Some(figure),
            _ => None,
        }
    }

    fn playing(&self) -> Option<&dyn Player> {
        match self {
            Self::Movie(movie) => Some(movie),
            Self::Music(music) => Some(music),
            Self::Figure(figure) if figure.animated() => Some(figure),
            _ => None,
        }
    }
}
