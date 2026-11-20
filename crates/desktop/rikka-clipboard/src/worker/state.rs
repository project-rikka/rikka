use std::os::fd::{AsFd as _, BorrowedFd};
use std::os::unix::io::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use rikka_wayland::{SerialKind, Serials};
use rikka_wayland_client::wl_data_device::{self, WlDataDevice};
use rikka_wayland_client::wl_data_device_manager::WlDataDeviceManager;
use rikka_wayland_client::wl_data_offer::{self, WlDataOffer};
use rikka_wayland_client::wl_data_source::{self, WlDataSource};
use rikka_wayland_client::zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1;
use rikka_wayland_client::zwp_primary_selection_device_v1::{self, ZwpPrimarySelectionDeviceV1};
use rikka_wayland_client::zwp_primary_selection_offer_v1::{self, ZwpPrimarySelectionOfferV1};
use rikka_wayland_client::zwp_primary_selection_source_v1::{self, ZwpPrimarySelectionSourceV1};

use super::content::Content;
use super::control::Control;
use crate::watch::{Change, Watcher};
use rikka_clipboard_core::{Error, Result, Selection};

pub(super) struct Published<S> {
    pub(super) content: Content,
    pub(super) source: Option<S>,
    pub(super) by: u64,
}

impl<S> Published<S> {
    pub(super) fn owner(&self) -> Option<u64> {
        self.source.is_some().then_some(self.by)
    }

    pub(super) fn open(&self, mime: &str) -> Option<Result<UnixStream>> {
        self.source.as_ref().and_then(|_| self.content.open(mime))
    }

    pub(super) fn mime_types_offered(&self) -> Option<Vec<String>> {
        self.source.as_ref().map(|_| self.content.mime_types())
    }
}

impl<S> Default for Published<S> {
    fn default() -> Self {
        Self {
            content: Content::default(),
            source: None,
            by: 0,
        }
    }
}

#[derive(Default)]
pub(super) struct Incoming {
    pub(super) offer: Option<WlDataOffer>,
    pub(super) mime_types: Vec<String>,
    pub(super) pending: Vec<String>,
    pub(super) primary_offer: Option<ZwpPrimarySelectionOfferV1>,
    pub(super) primary_mime_types: Vec<String>,
    pub(super) primary_pending: Vec<String>,
}

pub(super) struct State {
    pub(super) control: Option<Control>,
    pub(super) device: Option<WlDataDevice>,
    pub(super) primary_device: Option<ZwpPrimarySelectionDeviceV1>,
    pub(super) manager: Option<WlDataDeviceManager>,
    pub(super) primary_manager: Option<ZwpPrimarySelectionDeviceManagerV1>,
    pub(super) serials: Option<Serials>,
    pub(super) published: Published<WlDataSource>,
    pub(super) primary_published: Published<ZwpPrimarySelectionSourceV1>,
    pub(super) incoming: Incoming,
    pub(super) watchers: Vec<(u64, Watcher)>,
}

impl State {
    pub(super) fn new() -> Self {
        Self {
            control: None,
            device: None,
            primary_device: None,
            manager: None,
            primary_manager: None,
            serials: None,
            published: Published::default(),
            primary_published: Published::default(),
            incoming: Incoming::default(),
            watchers: Vec::new(),
        }
    }

    pub(super) fn serial(&self) -> u32 {
        self.serials
            .as_ref()
            .and_then(|serials| serials.latest(&SerialKind::ALL))
            .map_or(0, |latest| latest.serial)
    }

    pub(super) fn announce(
        &self,
        selection: Selection,
        owner: Option<u64>,
        mime_types: Vec<String>,
    ) {
        for (watching, watcher) in &self.watchers {
            let ours = owner == Some(*watching);
            watcher(&Change::new(selection, ours, mime_types.clone()));
        }
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("serials", &self.serials)
            .field("offered", &self.incoming.mime_types)
            .finish_non_exhaustive()
    }
}

pub(super) fn ensure_offered(types: &[String], mime: &str) -> Result<()> {
    if types.iter().any(|known| known == mime) {
        return Ok(());
    }
    Err(match types.is_empty() {
        true => Error::Empty,
        false => Error::NoSuchType(mime.to_owned()),
    })
}

pub(super) fn transfer(request_send: impl FnOnce(BorrowedFd<'_>)) -> Result<UnixStream> {
    let (ours, theirs) = UnixStream::pair().map_err(Error::Io)?;
    request_send(theirs.as_fd());
    drop(theirs);
    ours.shutdown(std::net::Shutdown::Write)
        .map_err(Error::Io)?;
    Ok(ours)
}

pub(super) fn pipe_with_data(data: Arc<[u8]>) -> Result<UnixStream> {
    let (ours, theirs) = UnixStream::pair().map_err(Error::Io)?;
    write_transfer(data, OwnedFd::from(theirs));
    Ok(ours)
}

pub(super) fn write_transfer(data: Arc<[u8]>, fd: OwnedFd) {
    let spawned = std::thread::Builder::new()
        .name("rikka-clipboard-send".into())
        .spawn(move || {
            use std::io::Write;
            let mut file = std::fs::File::from(fd);
            if let Err(err) = file.write_all(&data) {
                tracing::debug!(error = %err, "clipboard transfer interrupted");
            }
        });
    if let Err(err) = spawned {
        tracing::warn!(error = %err, "failed to start clipboard transfer");
    }
}

pub(super) fn on_wl_data_device(
    state: &mut State,
    _proxy: WlDataDevice,
    event: wl_data_device::Event,
) {
    match event {
        wl_data_device::Event::DataOffer { id: _ } => {
            state.incoming.pending.clear();
        }
        wl_data_device::Event::Enter { id, .. } => {
            state.incoming.pending.clear();
            if let Some(dragged) = id {
                dragged.destroy();
            }
        }
        wl_data_device::Event::Selection { id } => {
            if let Some(old) = state.incoming.offer.take() {
                old.destroy();
            }
            state.incoming.mime_types = std::mem::take(&mut state.incoming.pending);
            state.incoming.offer = id;
            if state.incoming.offer.is_none() {
                state.incoming.mime_types.clear();
            }
            let owner = state.published.owner();
            state.announce(
                Selection::Clipboard,
                owner,
                state.incoming.mime_types.clone(),
            );
        }
        _ => {}
    }
}

pub(super) fn on_wl_data_offer(
    state: &mut State,
    _proxy: WlDataOffer,
    event: wl_data_offer::Event,
) {
    if let wl_data_offer::Event::Offer { mime_type } = event {
        state.incoming.pending.push(mime_type);
    }
}

pub(super) fn on_wl_data_source(
    state: &mut State,
    proxy: WlDataSource,
    event: wl_data_source::Event,
) {
    match event {
        wl_data_source::Event::Send { mime_type, fd } => {
            state.published.content.send(&mime_type, fd);
        }
        wl_data_source::Event::Cancelled => {
            if state.published.source.as_ref() == Some(&proxy) {
                state.published = Published::default();
            }
            proxy.destroy();
        }
        _ => {}
    }
}

pub(super) fn on_zwp_primary_selection_device_v1(
    state: &mut State,
    _proxy: ZwpPrimarySelectionDeviceV1,
    event: zwp_primary_selection_device_v1::Event,
) {
    match event {
        zwp_primary_selection_device_v1::Event::DataOffer { offer: _ } => {
            state.incoming.primary_pending.clear();
        }
        zwp_primary_selection_device_v1::Event::Selection { id } => {
            if let Some(old) = state.incoming.primary_offer.take() {
                old.destroy();
            }
            state.incoming.primary_mime_types = std::mem::take(&mut state.incoming.primary_pending);
            state.incoming.primary_offer = id;
            if state.incoming.primary_offer.is_none() {
                state.incoming.primary_mime_types.clear();
            }
            let owner = state.primary_published.owner();
            state.announce(
                Selection::Primary,
                owner,
                state.incoming.primary_mime_types.clone(),
            );
        }
        _ => {}
    }
}

pub(super) fn on_zwp_primary_selection_offer_v1(
    state: &mut State,
    _proxy: ZwpPrimarySelectionOfferV1,
    event: zwp_primary_selection_offer_v1::Event,
) {
    if let zwp_primary_selection_offer_v1::Event::Offer { mime_type } = event {
        state.incoming.primary_pending.push(mime_type);
    }
}

pub(super) fn on_zwp_primary_selection_source_v1(
    state: &mut State,
    proxy: ZwpPrimarySelectionSourceV1,
    event: zwp_primary_selection_source_v1::Event,
) {
    match event {
        zwp_primary_selection_source_v1::Event::Send { mime_type, fd } => {
            state.primary_published.content.send(&mime_type, fd);
        }
        zwp_primary_selection_source_v1::Event::Cancelled => {
            if state.primary_published.source.as_ref() == Some(&proxy) {
                state.primary_published = Published::default();
            }
            proxy.destroy();
        }
        _ => {}
    }
}
