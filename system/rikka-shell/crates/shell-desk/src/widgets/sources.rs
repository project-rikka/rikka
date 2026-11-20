use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use desk_layout::WidgetId;
use rikka_mpris::Player;
use rikka_weather::TemperatureUnit;
use shell_feeds::Feeds;
use shell_kit::Feed;

use crate::widgets::audio_capture::AudioCapture;
use crate::widgets::monitor::History;
use crate::widgets::note::Drafts;
use crate::widgets::photos::Shelf;
use crate::widgets::power::Charging;

thread_local! {
    static SHARED: RefCell<Weak<Sources>> = const { RefCell::new(Weak::new()) };
}

pub(crate) struct Sources {
    pub(crate) feeds: Feeds,
    pub(crate) weather: Feed<rikka_weather::State>,
    pub(crate) unit: Feed<TemperatureUnit>,
    pub(crate) player: Feed<Option<Player>>,
    pub(super) audio_capture: AudioCapture,
    pub(crate) machine: Feed<History>,
    pub(crate) charging: Charging,
    pub(crate) drafts: Drafts,
    pub(super) pages: Feed<HashMap<WidgetId, i32>>,
    pub(crate) shelf: Shelf,
}

pub(super) struct Listening(Rc<Sources>);

impl Sources {
    #[must_use]
    pub(crate) fn shared(feeds: &Feeds) -> Rc<Sources> {
        SHARED.with(|shared| {
            if let Some(sources) = shared.borrow().upgrade() {
                return sources;
            }
            let sources = Rc::new(Sources::new(feeds.clone()));
            *shared.borrow_mut() = Rc::downgrade(&sources);
            sources
        })
    }

    fn new(feeds: Feeds) -> Sources {
        let unit = feeds.scale.get();
        Sources {
            feeds,
            weather: Feed::default(),
            unit: Feed::new(unit),
            player: Feed::default(),
            audio_capture: AudioCapture::default(),
            machine: Feed::default(),
            charging: Charging::default(),
            drafts: Drafts::default(),
            pages: Feed::default(),
            shelf: Shelf::default(),
        }
    }

    #[must_use]
    pub(crate) fn page(&self, widget: WidgetId) -> i32 {
        self.pages
            .with(|pages| pages.get(&widget).copied().unwrap_or_default())
    }
}

impl Listening {
    #[must_use]
    pub(crate) fn start(sources: &Rc<Sources>) -> Listening {
        sources.audio_capture.hold(&sources.feeds.audio_spectrum);
        Listening(Rc::clone(sources))
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        self.0.audio_capture.release();
    }
}

impl std::fmt::Debug for Sources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sources").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Listening {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listening").finish_non_exhaustive()
    }
}
