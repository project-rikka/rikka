use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures_channel::mpsc::UnboundedReceiver;
use rikka_input::evdev;
use rikka_nested::{Input, View};
use rikka_sync::Mutex;
use rikka_types::{Key, KeyCode, Point, Size};
use serde_json::json;

use crate::context_menu::EditCommand;
use crate::error::{Error, Result};
use crate::event::{CloseReason, Event};
use crate::ids::SessionId;
use crate::outbox::Outbox;
use crate::page::PageState;
use crate::runtime::Runtime;
use crate::script::{FrameResult, Run};

pub(crate) const DEFAULT_SIZE: Size = Size::new(1280.0, 720.0);

const FAVICON_SIZE: u32 = 64;

const INSPECT: [Key; 3] = [Key::ControlLeft, Key::ShiftLeft, Key::Char('i')];

#[must_use]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionSettings {
    pub(crate) url: Option<String>,
    pub(crate) size: Option<Size>,
}

impl SessionSettings {
    pub fn new() -> SessionSettings {
        SessionSettings::default()
    }

    pub fn url(mut self, url: impl Into<String>) -> SessionSettings {
        self.url = Some(url.into());
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> SessionSettings {
        self.size = Some(Size::new(width, height));
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub png: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Favicon {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

pub(crate) struct SessionCore {
    pub(crate) id: SessionId,
    pub(crate) view: View,
    page: Mutex<PageState>,
    pressed: Mutex<Option<Point>>,
    hovered_frame: Mutex<Option<i64>>,
    pub(crate) outbox: Outbox<Event>,
    ended: AtomicBool,
}

impl std::fmt::Debug for SessionCore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SessionCore")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl SessionCore {
    pub(crate) fn new(id: SessionId, view: View, url: String) -> SessionCore {
        SessionCore {
            id,
            view,
            page: Mutex::new(PageState {
                url,
                ..PageState::default()
            }),
            pressed: Mutex::new(None),
            hovered_frame: Mutex::new(None),
            outbox: Outbox::default(),
            ended: AtomicBool::new(false),
        }
    }

    pub(crate) fn page(&self) -> PageState {
        self.page.lock().clone()
    }

    pub(crate) fn update(&self, change: impl FnOnce(&mut PageState)) {
        let changed = {
            let mut page = self.page.lock();
            let before = page.clone();
            change(&mut page);
            *page != before
        };
        if changed {
            self.outbox.send(Event::Changed);
        }
    }

    pub(crate) fn press(&self, at: Option<Point>) {
        *self.pressed.lock() = at;
    }

    pub(crate) fn take_press(&self) -> Option<Point> {
        self.pressed.lock().take()
    }

    pub(crate) fn hover(&self, frame: i64, url: Option<String>) {
        let shown = {
            let mut hovered = self.hovered_frame.lock();
            match url {
                Some(_) => {
                    *hovered = Some(frame);
                    true
                }
                None if *hovered == Some(frame) => {
                    *hovered = None;
                    true
                }
                None => false,
            }
        };
        if shown {
            self.update(|page| page.hovered_link = url);
        }
    }

    pub(crate) fn clear_hover(&self) {
        *self.hovered_frame.lock() = None;
        self.update(|page| page.hovered_link = None);
    }

    pub(crate) fn send(&self, event: Event) {
        self.outbox.send(event);
    }

    pub(crate) fn end(&self, reason: CloseReason) {
        if self.ended.swap(true, Ordering::AcqRel) {
            return;
        }
        self.outbox.send(Event::Closed(reason));
        self.outbox.close();
    }

    pub(crate) fn has_ended(&self) -> bool {
        self.ended.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug)]
pub struct Session {
    core: Arc<SessionCore>,
    runtime: Runtime,
}

impl PartialEq for Session {
    fn eq(&self, other: &Session) -> bool {
        Arc::ptr_eq(&self.core, &other.core)
    }
}

impl Eq for Session {}

impl Hash for Session {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.core).hash(state);
    }
}

impl Session {
    pub(crate) fn new(core: Arc<SessionCore>, runtime: Runtime) -> Session {
        Session { core, runtime }
    }

    pub(crate) fn core(&self) -> &Arc<SessionCore> {
        &self.core
    }

    #[must_use]
    pub fn id(&self) -> SessionId {
        self.core.id
    }

    #[must_use]
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    #[must_use]
    pub fn events(&self) -> Option<UnboundedReceiver<Event>> {
        self.core.outbox.open()
    }

    #[must_use]
    pub fn page(&self) -> PageState {
        self.core.page()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.core.has_ended()
    }

    pub fn load(&self, url: impl Into<String>) {
        let url = url.into();
        self.core.update(|page| page.url.clone_from(&url));
        self.runtime.core().load(self.core.id, url);
    }

    pub fn reload(&self) {
        self.tab_call("tabs.reload", |tab| {
            vec![json!(tab), json!({ "bypassCache": false })]
        });
    }

    pub fn reload_bypassing_cache(&self) {
        self.tab_call("tabs.reload", |tab| {
            vec![json!(tab), json!({ "bypassCache": true })]
        });
    }

    pub fn stop(&self) {
        self.control("stop", 0);
    }

    pub fn back(&self) {
        self.travel(-1);
    }

    pub fn forward(&self) {
        self.travel(1);
    }

    pub fn go(&self, delta: i32) {
        self.control("go", delta);
    }

    pub fn set_muted(&self, muted: bool) {
        self.tab_call("tabs.update", |tab| {
            vec![json!(tab), json!({ "muted": muted })]
        });
    }

    pub fn set_zoom(&self, zoom: f64) {
        self.tab_call("tabs.setZoom", |tab| vec![json!(tab), json!(zoom)]);
    }

    pub fn discard(&self) {
        self.tab_call("tabs.discard", |tab| vec![json!(tab)]);
    }

    pub fn print(&self) {
        self.control("print", 0);
    }

    pub fn exit_fullscreen(&self) {
        self.control("exit-fullscreen", 0);
    }

    pub fn edit(&self, frame: i64, command: EditCommand) {
        let (name, text) = command.exec();
        let text = text.map(str::to_owned);
        self.tab_call("rikka.edit", move |tab| {
            vec![json!({ "tab": tab, "frame": frame, "command": name, "text": text })]
        });
    }

    pub fn selection(&self, frame: i64) -> impl Future<Output = Result<String>> + Send + use<> {
        let core = self.runtime.core();
        let called = core.tab(self.core.id).map(|tab| {
            core.call(
                "rikka.selection",
                vec![json!({ "tab": tab.0, "frame": frame })],
            )
        });
        async move { called?.decoded().await }
    }

    pub fn inspect(&self) {
        let codes: Vec<KeyCode> = INSPECT
            .into_iter()
            .filter_map(evdev::key_code)
            .map(KeyCode::from_evdev)
            .collect();
        let view = &self.core.view;
        view.deliver(Input::Focus(true));
        for code in &codes {
            view.deliver(Input::Key {
                code: *code,
                pressed: true,
            });
        }
        for code in codes.iter().rev() {
            view.deliver(Input::Key {
                code: *code,
                pressed: false,
            });
        }
    }

    pub fn insert_style(&self, css: &str) {
        let css = css.to_owned();
        self.tab_call("scripting.insertCSS", move |tab| {
            vec![json!({ "target": { "tabId": tab, "allFrames": true }, "css": css, "origin": "USER" })]
        });
    }

    pub fn remove_style(&self, css: &str) {
        let css = css.to_owned();
        self.tab_call("scripting.removeCSS", move |tab| {
            vec![json!({ "target": { "tabId": tab, "allFrames": true }, "css": css, "origin": "USER" })]
        });
    }

    pub fn set_visible(&self, visible: bool) {
        self.core.view.set_visible(visible);
    }

    pub fn close(&self) {
        self.runtime.core().close(self.core.id);
    }

    pub fn run(&self, run: Run) -> impl Future<Output = Result<Vec<FrameResult>>> + Send + use<> {
        let called = self.runtime.core().tab(self.core.id).map(|tab| {
            self.runtime
                .core()
                .call("rikka.run", vec![run.request(tab.0)])
        });
        async move { called?.decoded().await }
    }

    pub fn capture(&self) -> impl Future<Output = Result<Snapshot>> + Send + use<> {
        let core = self.runtime.core();
        let called = core.window(self.core.id).map(|window| {
            core.call(
                "tabs.captureVisibleTab",
                vec![json!(window), json!({ "format": "png" })],
            )
        });
        async move {
            let url: String = called?.decoded().await?;
            let encoded = url
                .strip_prefix("data:image/png;base64,")
                .ok_or_else(|| Error::Malformed("capture is not a PNG data URL".into()))?;
            let png = STANDARD
                .decode(encoded)
                .map_err(|error| Error::Malformed(error.to_string()))?;
            Ok(Snapshot { png })
        }
    }

    pub fn favicon(&self) -> impl Future<Output = Result<Favicon>> + Send + use<> {
        let page = self.core.page().url;
        let called = self.runtime.core().call(
            "rikka.favicon",
            vec![json!({ "page": page, "size": FAVICON_SIZE })],
        );
        async move {
            #[derive(serde::Deserialize)]
            struct Fetched {
                #[serde(rename = "type")]
                mime_type: String,
                data: String,
            }
            let fetched: Fetched = called.decoded().await?;
            let bytes = STANDARD
                .decode(fetched.data)
                .map_err(|error| Error::Malformed(error.to_string()))?;
            Ok(Favicon {
                mime_type: fetched.mime_type,
                bytes,
            })
        }
    }

    fn tab_call(&self, api: &'static str, args: impl FnOnce(i64) -> Vec<serde_json::Value>) {
        let core = self.runtime.core();
        match core.tab(self.core.id) {
            Ok(tab) => core.notify(api, args(tab.0)),
            Err(error) => tracing::debug!(%error, api, "session cannot take the command yet"),
        }
    }

    fn control(&self, action: &'static str, delta: i32) {
        self.tab_call("rikka.control", move |tab| {
            vec![json!({ "tab": tab, "action": action, "delta": delta })]
        });
    }

    fn travel(&self, delta: i32) {
        self.tab_call("rikka.travel", move |tab| {
            vec![json!({ "tab": tab, "delta": delta })]
        });
    }
}
