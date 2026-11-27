use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::Instant;

use rikka_clipboard::Offer;
use rikka_compositor_reactor::Sender;
use rikka_compositor_selection::SelectionKind;
use rikka_picture::{SharedGpu, Tick};
use rikka_types::Size;

use crate::claim::Claim;
use crate::input::Input;
use crate::server::HoldId;
use crate::shared::Shared;
use crate::view::ViewId;

pub(crate) enum Command {
    Adopt {
        view: ViewId,
        claim: Claim,
        shared: Arc<Shared>,
    },
    Forget(ViewId),
    Reclaim {
        view: ViewId,
        claim: Claim,
    },
    Resize {
        view: ViewId,
        size: Size,
        scale: f64,
    },
    Visible {
        view: ViewId,
        visible: bool,
    },
    Fullscreen {
        view: ViewId,
        fullscreen: bool,
    },
    Close(ViewId),
    Input {
        view: ViewId,
        input: Input,
        at: Instant,
    },
    Tick {
        view: ViewId,
        tick: Tick,
    },
    Released(HoldId),
    SessionSelection {
        kind: SelectionKind,
        mime_types: Vec<String>,
    },
    ClientSend {
        kind: SelectionKind,
        generation: u64,
        mime_type: String,
        fd: OwnedFd,
    },
    DragRead {
        generation: u64,
        offer: Offer,
    },
    Gpu(Option<SharedGpu>),
    Stop,
}

pub(crate) type Link = Sender<Command>;
