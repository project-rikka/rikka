use std::any::Any;
use std::cell::RefCell;
use std::sync::Arc;

use rikka_clipboard_core::{Offer, URI_LIST, text_mime, uri_list};
use rikka_theme::Theme;
use rikka_types::{DragAction, DragActions, Point, Size};

use crate::view::Element;

pub const DRAG_SLOP: f32 = 5.0;

pub struct DragStart {
    pub offer: Offer,
    pub actions: DragActions,
    pub icon: DragIcon,
    pub moves_windows: bool,
}

impl DragStart {
    #[must_use]
    pub fn new(offer: Offer, icon: DragIcon) -> Self {
        Self {
            offer,
            actions: DragActions::COPY,
            icon,
            moves_windows: false,
        }
    }

    #[must_use]
    pub fn actions(mut self, actions: DragActions) -> Self {
        self.actions = actions;
        self
    }

    #[must_use]
    pub fn moves_windows(mut self, moves_windows: bool) -> Self {
        self.moves_windows = moves_windows;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragWindow {
    Opening { grip: Point },
    This { grip: Point },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accept {
    pub mime: String,
    pub actions: DragActions,
    pub preferred: DragAction,
}

impl Accept {
    #[must_use]
    pub fn new(mime: impl Into<String>, actions: DragActions, preferred: DragAction) -> Self {
        Self {
            mime: mime.into(),
            actions,
            preferred,
        }
    }

    #[must_use]
    pub fn copy(mime: impl Into<String>) -> Self {
        Self::new(mime, DragActions::COPY, DragAction::Copy)
    }

    #[must_use]
    pub fn moving(mime: impl Into<String>) -> Self {
        Self::new(mime, DragActions::MOVE, DragAction::Move)
    }

    #[must_use]
    pub fn copy_or_move(mime: impl Into<String>, offer: &DragOffer) -> Self {
        let moves = offer.is_ours() && offer.source_actions().contains(DragAction::Move);
        let preferred = match moves {
            true => DragAction::Move,
            false => DragAction::Copy,
        };
        Self::new(mime, DragActions::COPY_OR_MOVE, preferred)
    }
}

type Draw = Box<dyn Fn() -> Element>;

pub struct DragIcon {
    pub size: Size,
    pub hotspot: Point,
    theme: Theme,
    draw: Draw,
}

impl DragIcon {
    pub fn new(
        size: Size,
        hotspot: Point,
        theme: Theme,
        draw: impl Fn() -> Element + 'static,
    ) -> Self {
        Self {
            size,
            hotspot,
            theme,
            draw: Box::new(draw),
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    #[must_use]
    pub fn draw(&self) -> Element {
        (self.draw)()
    }
}

pub trait Delivery: Send + Sync + 'static {
    fn read(&self, mime: &str) -> std::io::Result<Vec<u8>>;
}

#[derive(Clone)]
pub struct DragOffer {
    mimes: Arc<[String]>,
    actions: DragActions,
    ours: bool,
    delivery: Option<Arc<dyn Delivery>>,
}

impl DragOffer {
    #[must_use]
    pub fn new(mimes: impl IntoIterator<Item = String>) -> Self {
        Self {
            mimes: mimes.into_iter().collect(),
            actions: DragActions::COPY_OR_MOVE,
            ours: false,
            delivery: None,
        }
    }

    #[must_use]
    pub fn actions(mut self, actions: DragActions) -> Self {
        self.actions = actions;
        self
    }

    #[must_use]
    pub fn ours(mut self, ours: bool) -> Self {
        self.ours = ours;
        self
    }

    #[must_use]
    pub fn delivered_by(mut self, delivery: impl Delivery) -> Self {
        self.delivery = Some(Arc::new(delivery));
        self
    }

    #[must_use]
    pub fn offers(&self, mime: &str) -> bool {
        self.mimes.iter().any(|offered| offered == mime)
    }

    #[must_use]
    pub fn mimes(&self) -> &[String] {
        &self.mimes
    }

    #[must_use]
    pub fn source_actions(&self) -> DragActions {
        self.actions
    }

    #[must_use]
    pub fn is_ours(&self) -> bool {
        self.ours
    }

    #[must_use]
    pub fn first_of<'a>(&self, candidates: &[&'a str]) -> Option<&'a str> {
        candidates.iter().copied().find(|mime| self.offers(mime))
    }

    #[must_use]
    pub fn text_mime(&self) -> Option<&str> {
        text_mime(&self.mimes)
    }

    #[must_use]
    pub fn has_uris(&self) -> bool {
        self.offers(URI_LIST)
    }

    pub fn read(&self, mime: &str) -> Option<Vec<u8>> {
        let delivery = self.delivery.as_ref()?;
        match delivery.read(mime) {
            Ok(data) => Some(data),
            Err(error) => {
                tracing::warn!("failed to read dropped `{mime}`: {error}");
                None
            }
        }
    }

    pub fn text(&self) -> Option<String> {
        let mime = self.text_mime()?.to_owned();
        let bytes = self.read(&mime)?;
        match String::from_utf8(bytes) {
            Ok(text) => Some(text),
            Err(error) => Some(String::from_utf8_lossy(error.as_bytes()).into_owned()),
        }
    }

    pub fn uris(&self) -> Vec<String> {
        if !self.has_uris() {
            return Vec::new();
        }
        let Some(bytes) = self.read(URI_LIST) else {
            return Vec::new();
        };
        uri_list(&String::from_utf8_lossy(&bytes))
    }
}

impl PartialEq for DragOffer {
    fn eq(&self, other: &Self) -> bool {
        self.mimes == other.mimes
            && self.actions == other.actions
            && self.ours == other.ours
            && match (&self.delivery, &other.delivery) {
                (Some(ours), Some(theirs)) => Arc::ptr_eq(ours, theirs),
                (None, None) => true,
                _ => false,
            }
    }
}

impl std::fmt::Debug for DragOffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DragOffer")
            .field("mimes", &self.mimes)
            .field("actions", &self.actions)
            .field("ours", &self.ours)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragEnd {
    Landed(DragAction),
    Nowhere,
    Aborted,
}

thread_local! {
    static CARGO: RefCell<Option<Box<dyn Any>>> = const { RefCell::new(None) };
}

pub fn set_drag_payload(cargo: impl Any) {
    CARGO.with_borrow_mut(|cargo_slot| *cargo_slot = Some(Box::new(cargo)));
}

#[must_use]
pub fn take_drag_payload<T: Any>() -> Option<T> {
    CARGO.with_borrow_mut(|cargo_slot| {
        let cargo = cargo_slot.take()?;
        match cargo.downcast::<T>() {
            Ok(value) => Some(*value),
            Err(other) => {
                *cargo_slot = Some(other);
                None
            }
        }
    })
}

pub fn clear_drag_payload() {
    CARGO.with_borrow_mut(|cargo_slot| *cargo_slot = None);
}

pub fn has_drag_payload() -> bool {
    CARGO.with_borrow(Option::is_some)
}

impl std::fmt::Debug for DragStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DragStart")
            .field("offer", &self.offer)
            .field("actions", &self.actions)
            .field("icon", &self.icon)
            .field("moves_windows", &self.moves_windows)
            .finish()
    }
}

impl std::fmt::Debug for DragIcon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DragIcon")
            .field("size", &self.size)
            .field("hotspot", &self.hotspot)
            .finish_non_exhaustive()
    }
}
