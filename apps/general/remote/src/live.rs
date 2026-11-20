use std::collections::HashMap;
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_input::cursor::{CursorPicture, define_cursor, forget_cursor};
use rikka_remote_link::Pin;
use rikka_remote_wire::{
    CURSOR_SHAPES, Codec, CursorShape as RemoteCursor, EndReason, HIDDEN_CURSOR, Refusal, Stats,
    Stream,
};
use rikka_types::CursorImage;
use rikka_ui::Feed;
use rikka_widgets::{column, row};
use viewer_hosts::Host;

use crate::screen::{Escape, Sprite, screen};
use crate::session::{Command, Link, Refused, Report};
use crate::viewer::Viewer;

const FORM: f32 = 320.0;
const MEGABIT: f64 = 1_000_000.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ending {
    Reason(EndReason),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Connecting,
    Password(Option<Refused>),
    Streaming,
    Changed(Pin),
    Ended(Ending),
}

struct Remembered {
    image: Option<CursorImage>,
    sprite: Sprite,
    used: u64,
}

pub(crate) struct Live {
    link: Link,
    feed: Feed,
    pub(crate) host: Host,
    pub(crate) phase: Phase,
    pub(crate) seen: Option<Pin>,
    user: String,
    display: Option<(u32, u32)>,
    stream: Option<Stream>,
    stats: Option<Stats>,
    stats_shown: bool,
    cursors: HashMap<u32, Remembered>,
    shown_cursors: u64,
    cursor: CursorShape,
    sprite: Option<Sprite>,
    pointer: Option<Point>,
    pub(crate) password: TextDocument,
    pub(crate) remember: bool,
    notice: Option<String>,
}

impl Live {
    pub(crate) fn new(link: Link, feed: Feed, host: Host) -> Live {
        Live {
            link,
            feed,
            host,
            phase: Phase::Connecting,
            seen: None,
            user: String::new(),
            display: None,
            stream: None,
            stats: None,
            stats_shown: false,
            cursors: HashMap::new(),
            shown_cursors: 0,
            cursor: CursorShape::Default,
            sprite: None,
            pointer: None,
            password: TextDocument::new(""),
            remember: true,
            notice: None,
        }
    }

    pub(crate) fn send(&self, command: Command) {
        self.link.send(command);
    }

    pub(crate) fn toggle_stats(&mut self) {
        self.stats_shown = !self.stats_shown;
    }

    pub(crate) fn report(&mut self, report: Report) {
        match report {
            Report::Connected { host, user, pin } => {
                self.host.name = host;
                self.user = user;
                self.seen = pin;
            }
            Report::Password(refused) => {
                self.password.reset("");
                self.phase = Phase::Password(refused);
            }
            Report::TokenRefused => {}
            Report::SignedIn { .. } => self.phase = Phase::Streaming,
            Report::Display(display) => self.display = Some((display.width, display.height)),
            Report::Stream(stream) => self.stream = Some(stream),
            Report::CursorShape(shape) => self.define(shape),
            Report::Cursor(serial) => self.show_cursor(serial),
            Report::Pointer { x, y } => self.pointer = Some(Point::new(x as f32, y as f32)),
            Report::Stats(stats) => self.stats = Some(stats),
            Report::FileSaved(name) => {
                self.notice = Some(tr!("remote.saved", name = name.as_str()).to_string());
            }
            Report::FileFailed(reason) => {
                self.notice =
                    Some(tr!("remote.upload-failed", reason = reason.as_str()).to_string());
            }
            Report::PinChanged { seen } => self.phase = Phase::Changed(seen),
            Report::Ended(reason) => self.phase = Phase::Ended(Ending::Reason(reason)),
            Report::Failed(reason) => {
                if self.phase != Phase::Ended(Ending::Reason(EndReason::SignedOut)) {
                    self.phase = Phase::Ended(Ending::Failed(reason));
                }
            }
        }
    }

    fn show_cursor(&mut self, serial: u32) {
        self.shown_cursors += 1;
        let shown = self.shown_cursors;
        let remembered = (serial != HIDDEN_CURSOR)
            .then(|| self.cursors.get_mut(&serial))
            .flatten();
        self.sprite = remembered
            .as_ref()
            .map(|remembered| remembered.sprite.clone());
        self.cursor = match remembered {
            Some(remembered) => {
                remembered.used = shown;
                remembered
                    .image
                    .map_or(CursorShape::Default, CursorShape::Image)
            }
            None if serial == HIDDEN_CURSOR => CursorShape::Hidden,
            None => CursorShape::Default,
        };
    }

    fn define(&mut self, shape: RemoteCursor) {
        self.shown_cursors += 1;
        let sprite = Sprite {
            width: u32::from(shape.width),
            height: u32::from(shape.height),
            hotspot: Point::new(f32::from(shape.hotspot_x), f32::from(shape.hotspot_y)),
            pixels: Arc::from(shape.rgba.as_slice()),
        };
        let image = define_cursor(CursorPicture {
            width: sprite.width,
            height: sprite.height,
            hotspot_x: i32::from(shape.hotspot_x),
            hotspot_y: i32::from(shape.hotspot_y),
            rgba: shape.rgba,
        });
        let remembered = Remembered {
            image,
            sprite,
            used: self.shown_cursors,
        };
        if let Some(replaced) = self.cursors.insert(shape.serial, remembered) {
            replaced.forget();
        }
        while self.cursors.len() > CURSOR_SHAPES {
            let oldest = self
                .cursors
                .iter()
                .min_by_key(|(_, remembered)| remembered.used)
                .map(|(serial, _)| *serial);
            if let Some(remembered) = oldest.and_then(|serial| self.cursors.remove(&serial)) {
                remembered.forget();
            }
        }
    }
}

impl Remembered {
    fn forget(self) {
        if let Some(image) = self.image {
            forget_cursor(image);
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        for (_, remembered) in self.cursors.drain() {
            remembered.forget();
        }
    }
}

impl std::fmt::Debug for Live {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Live")
            .field("host", &self.host.address)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(live: &Live, fullscreen: bool) -> impl View {
    match &live.phase {
        Phase::Streaming => streaming(live, fullscreen).any(),
        Phase::Connecting => centred(
            column! {
                spinner();
                text(tr!("remote.connecting", host = live.host.name.as_str())).label();
                button(tr!("remote.cancel"))
                    .secondary()
                    .on_press(Viewer::leave);
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center),
        )
        .any(),
        Phase::Password(refused) => centred(sign_in(live, refused.as_ref())).any(),
        Phase::Changed(seen) => centred(changed(live, *seen)).any(),
        Phase::Ended(ending) => centred(ended(ending)).any(),
    }
}

fn centred(content: impl View) -> impl View {
    column! {
        content;
    }
    .align_items(Align::Center)
    .justify(Justify::Center)
    .padding(Space::WIDE)
    .fill()
}

fn sign_in(live: &Live, refused: Option<&Refused>) -> impl View {
    let problem = refused.map(|refused| match refused.reason {
        Refusal::Throttled => tr!(
            "remote.sign-in.throttled",
            seconds = refused.retry_after_ms.div_ceil(1000)
        ),
        Refusal::Unavailable => {
            tr!(
                "remote.sign-in.unavailable",
                detail = refused.detail.as_str()
            )
        }
        Refusal::WrongPassword | Refusal::UnknownToken => tr!("remote.sign-in.wrong"),
    });
    column! {
        text(tr!("remote.sign-in.heading", host = live.host.name.as_str())).title();
        problem.map(|problem| text(problem).caption().color(TextColor::Danger));
        column! {
            text(tr!("remote.sign-in.password", user = live.user.as_str())).label().muted();
            text_field(&live.password)
                .secret(true)
                .autofocus(true)
                .on_change(|| {})
                .on_submit(Viewer::sign_in)
                .fill_width();
        }
        .gap(Space::HAIR)
        .fill_width();
        field(
            tr!("remote.sign-in.remember"),
            switch(live.remember).on_toggle(|viewer: &mut Viewer, on: bool| viewer.set_remember(on)),
        );
        row! {
            button(tr!("remote.cancel")).secondary().on_press(Viewer::leave);
            button(tr!("remote.sign-in.submit"))
                .primary()
                .enabled(!live.password.is_empty())
                .on_press(Viewer::sign_in);
        }
        .gap(Space::SNUG);
    }
    .gap(Space::NORMAL)
    .width(FORM * 1.5)
}

fn changed(live: &Live, seen: Pin) -> impl View {
    column! {
        text(tr!("remote.changed.heading")).title().color(TextColor::Warning);
        text(tr!("remote.changed.body", host = live.host.name.as_str())).label();
        text(tr!("remote.changed.seen", fingerprint = seen.to_string().as_str()))
            .caption()
            .family(Family::parse(font::JETBRAINS_MONO));
        row! {
            button(tr!("remote.cancel")).secondary().on_press(Viewer::leave);
            button(tr!("remote.changed.trust"))
                .danger()
                .on_press(move |viewer: &mut Viewer| viewer.trust(seen));
        }
        .gap(Space::SNUG);
    }
    .gap(Space::NORMAL)
    .width(FORM * 1.5)
}

fn ended(ending: &Ending) -> impl View {
    let message = match ending {
        Ending::Reason(EndReason::Replaced) => tr!("remote.ended.replaced"),
        Ending::Reason(EndReason::Stopped) => tr!("remote.ended.stopped"),
        Ending::Reason(EndReason::SignedOut) => tr!("remote.ended.signed-out"),
        Ending::Reason(EndReason::Disconnected) => tr!("remote.ended.disconnected"),
        Ending::Reason(EndReason::Unsupported) => tr!("remote.ended.unsupported"),
        Ending::Failed(reason) => tr!("remote.ended.failed", reason = reason.as_str()),
    };
    column! {
        text(message).label();
        row! {
            button(tr!("remote.back")).secondary().on_press(Viewer::leave);
            button(tr!("remote.reconnect")).primary().on_press(Viewer::reconnect);
        }
        .gap(Space::SNUG);
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
}

fn streaming(live: &Live, fullscreen: bool) -> impl View {
    column! {
        if !fullscreen {
            bar(live);
        }
        screen(&live.feed, live.link.commands())
            .display(live.display)
            .cursor(live.cursor)
            .drawn_cursor(live.sprite.clone(), live.pointer)
            .on_escape(|viewer: &mut Viewer, escape: Escape| viewer.escape(escape))
            .fill();
    }
    .fill()
}

fn bar(live: &Live) -> impl View {
    let reading = live
        .stats_shown
        .then(|| live.stats.zip(live.stream.as_ref()))
        .flatten()
        .map(|(stats, stream)| statistics(&stats, stream));
    let mut bar = toolbar()
        .padding(Space::TIGHT)
        .control_size(ControlSize::Medium)
        .group(
            tool_group([
                tool(
                    MaterialSymbol::CLOSE,
                    tr!("remote.toolbar.disconnect"),
                    Viewer::leave,
                ),
                tool(
                    MaterialSymbol::FULLSCREEN,
                    tr!("remote.toolbar.fullscreen"),
                    || request_window(WindowRequest::ToggleFullscreen),
                ),
            ])
            .priority(ToolPriority::Pinned),
        )
        .group(tool_group([
            tool(
                MaterialSymbol::REFRESH,
                tr!("remote.toolbar.keyframe"),
                |viewer: &mut Viewer| viewer.send(Command::Keyframe),
            ),
            tool(
                MaterialSymbol::QUERY_STATS,
                tr!("remote.stats-toggle"),
                |viewer: &mut Viewer| viewer.escape(Escape::Stats),
            ),
        ]));
    if let Some(reading) = reading {
        bar = bar.group(
            tool_group([tool_item(text(reading).caption().muted().no_wrap())])
                .priority(ToolPriority::Low),
        );
    }
    if let Some(notice) = &live.notice {
        bar = bar.group(tool_group([tool_item(
            text(notice.clone()).caption().no_wrap(),
        )
        .overflow([menu_item(notice.clone()).icon(MaterialSymbol::INFO)])]));
    }
    bar.spacer()
        .group(
            tool_group([tool_item(
                text(tr!("remote.chords")).caption().muted().no_wrap(),
            )])
            .priority(ToolPriority::Low),
        )
        .group(
            tool_group([tool(
                MaterialSymbol::LOGOUT,
                tr!("remote.toolbar.sign-out"),
                |viewer: &mut Viewer| viewer.send(Command::SignOut),
            )])
            .priority(ToolPriority::High),
        )
}

fn tool<Marker>(
    symbol: MaterialSymbol,
    label: Label,
    press: impl rikka_ui::IntoHandler<(), (), Marker>,
) -> ToolButton {
    tool_button(symbol, label)
        .control_size(ControlSize::Medium)
        .on_press(press)
}

fn statistics(stats: &Stats, stream: &Stream) -> String {
    let codec = match stream.codec {
        Codec::H264 => "H.264",
        Codec::H265 => "H.265",
        Codec::Av1 => "AV1",
    };
    tr!(
        "remote.stats",
        codec = codec,
        width = stream.width,
        height = stream.height,
        megabits = format!("{:.1}", stats.target_bps as f64 / MEGABIT).as_str(),
        rtt = stats.rtt_us / 1000,
        loss = format!("{:.1}", f64::from(stats.loss_permille) / 10.0).as_str()
    )
    .to_string()
}
