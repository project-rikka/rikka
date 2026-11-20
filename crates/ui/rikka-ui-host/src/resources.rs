use std::sync::Once;

use rikka_image::ColorScheme;
use rikka_picture::consume::VideoConsumer;
use rikka_picture::{Changes, Pulse, VideoController, VideoState};
use rikka_renderer::{TextureId, Uploads};
use rikka_theme::Theme;
use rikka_ui::{ImageSource, SourceKey, TextureResolver};

use crate::cache::ImageCache;
use crate::video::{VideoFrame, VideoPlayback};

const NO_PLAYBACK: &str = "this program does not play video";

#[derive(Debug, Default)]
pub struct Resources {
    images: ImageCache,
    videos: Option<Box<dyn VideoPlayback>>,
}

#[derive(Clone, Debug, Default)]
pub struct ResourceFrame {
    pub settled: Vec<SourceKey>,
    pub updated: Vec<TextureId>,
    pub repainted: bool,
    pub loading: bool,
    pub due: Option<std::time::Duration>,
    pub pulsed: Vec<(SourceKey, Pulse)>,
}

impl Resources {
    #[must_use]
    pub fn new() -> Resources {
        Resources::default()
    }

    pub fn plays_video(&mut self, videos: Box<dyn VideoPlayback>) {
        self.videos = Some(videos);
    }

    #[must_use]
    pub fn wants_commit(&self) -> bool {
        self.images.has_work() || self.videos.as_ref().is_some_and(|videos| videos.has_work())
    }

    pub fn shown<'a>(&mut self, drawn: impl Iterator<Item = &'a SourceKey>) {
        self.images.shown(drawn);
    }

    pub fn release(&mut self, uploads: &Uploads) {
        self.images.release(uploads);
    }

    pub(crate) fn follow_theme(&mut self, theme: &Theme) {
        self.images.set_color_scheme(match theme.is_dark() {
            true => ColorScheme::Dark,
            false => ColorScheme::Light,
        });
    }

    pub fn commit(&mut self, uploads: &Uploads) -> ResourceFrame {
        let (video, starting) = match &mut self.videos {
            Some(videos) => (videos.advance(uploads), videos.is_starting()),
            None => (VideoFrame::default(), false),
        };
        let mut frame = ResourceFrame {
            settled: video.settled,
            updated: video.updated,
            repainted: false,
            due: video.due,
            loading: starting,
            pulsed: Vec::new(),
        };
        self.images
            .commit(uploads, &mut frame.settled, &mut frame.updated);
        frame.loading |= self.images.has_backlog();
        frame.repainted = self.images.take_repainted();
        frame.pulsed = self.images.take_pulsed();
        frame
    }

    fn resolve_video(&mut self, controller: &VideoController) -> Option<TextureId> {
        match &mut self.videos {
            Some(videos) => videos.resolve(controller),
            None => {
                if controller.state() != VideoState::Failed {
                    controller.fail(NO_PLAYBACK.to_owned());
                }
                None
            }
        }
    }
}

impl TextureResolver for Resources {
    fn resolve(&mut self, source: &ImageSource) -> Option<TextureId> {
        match source {
            ImageSource::Video(controller) => self.resolve_video(controller),
            _ => self.images.resolver().resolve(source),
        }
    }

    fn changes(&mut self, source: &ImageSource, since: u64) -> Changes {
        match source {
            ImageSource::Video(_) => Changes::UNKNOWN,
            _ => self.images.resolver().changes(source, since),
        }
    }
}

pub(crate) fn warn_video_unresolved() {
    static WARNED: Once = Once::new();
    WARNED.call_once(|| {
        tracing::warn!("video drawn without `Resources`");
    });
}
