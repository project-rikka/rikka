use std::time::Instant;

use rikka_appearance::Appearance;
use rikka_instance::LaunchRequest;
use rikka_renderer::{Renderer, SurfaceConfig};
use rikka_swapchain::Presenter;
use rikka_theme::Theme;
use rikka_types::{CursorShape, Point, Size, Stylus};
use rikka_ui::{Event, WindowPart, WindowRef};
use rikka_worker::EventFd;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Ole::DROPEFFECT;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::HRESULT;

use crate::device::Device;
use crate::error::{Error, Result};
use crate::instance::Policy;
use crate::launch::Launch;
use crate::tasks::{LoopEvent, LoopProxy, LoopReceiver, LoopSink, WindowId};
use crate::win32::clock::Clock;
use crate::win32::drag::{DragLoop, Dragging};
use crate::win32::heard::{self, Heard};
use crate::win32::hosting::{self, Hosting, Windows};
use crate::win32::input::cursor::Cursors;
use crate::win32::monitor;
use crate::win32::open::{self, Opener};
use crate::win32::window::dwm::Look;
use crate::win32::window::handle::WindowClass;
use crate::win32::window::state::{Modal, ModalCall, bring_forward};
use crate::win32::window::{placement, show_pointer};
use crate::window::{self, Opening};

pub(crate) struct Runner {
    app_name: String,
    device: Device,
    theme: Theme,
    appearance: Option<Appearance>,
    pending: Vec<Opening>,
    hosting: Hosting,
    dragging: Dragging,
    policy: Option<Policy>,
    proxy: LoopProxy,
    mailbox: LoopReceiver,
    wake: EventFd,
    class: WindowClass,
    clock: Clock,
    cursors: Cursors,
    next_id: WindowId,
    touched: Vec<WindowId>,
    heard: Vec<(WindowId, Heard)>,
    modals: Vec<(WindowId, Modal)>,
    exiting: bool,
    error: Option<Error>,
}

impl Runner {
    pub(crate) fn new(launch: Launch, class: WindowClass, clock: Clock) -> Runner {
        let Launch {
            app_name,
            device,
            videos,
            theme,
            appearance,
            windows,
            policy,
            proxy,
            mailbox,
            wake,
        } = launch;
        let mut hosting = Hosting::new(LoopSink::new(proxy.clone()));
        let waking = proxy.clone();
        hosting.wakes_with(std::sync::Arc::new(move || waking.wake()));
        if let Some(videos) = videos {
            hosting.plays_video(videos);
        }
        Runner {
            app_name,
            device,
            theme,
            appearance,
            pending: windows,
            hosting,
            dragging: Dragging::default(),
            policy,
            proxy,
            mailbox,
            wake,
            class,
            clock,
            cursors: Cursors::default(),
            next_id: WindowId::from(1),
            touched: Vec::new(),
            heard: Vec::new(),
            modals: Vec::new(),
            exiting: false,
            error: None,
        }
    }

    pub(crate) fn start(&mut self) -> Result<()> {
        window::accept_window_requests();
        self.open_pending()?;
        if self.hosting.is_empty() {
            self.exiting = true;
        }
        Ok(())
    }

    pub(crate) fn exiting(&self) -> bool {
        self.exiting
    }

    pub(crate) fn wait_handles(&self) -> [HANDLE; 2] {
        use std::os::windows::io::{AsHandle as _, AsRawHandle as _};
        [
            HANDLE(self.wake.as_handle().as_raw_handle()),
            self.clock.handle(),
        ]
    }

    pub(crate) fn woke(&self) {
        self.wake.drain();
    }

    pub(crate) fn has_heard(&self) -> bool {
        !heard::is_empty()
    }

    pub(crate) fn finish(mut self) -> Result<()> {
        self.dragging.release();
        self.hosting.shutdown();
        match self.error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(crate) fn fail(&mut self, error: Error) {
        tracing::error!("fatal error: {error}");
        if self.error.is_none() {
            self.error = Some(error);
        }
        self.exiting = true;
    }

    pub(crate) fn window_part(&self, id: WindowId, point: Point) -> Option<WindowPart> {
        self.hosting.get(&id)?.ui.window_part_at(point)
    }

    pub(crate) fn take_modal(&mut self) -> Option<ModalCall> {
        while !self.modals.is_empty() {
            let (id, modal) = self.modals.remove(0);
            if let Some(hosted) = self.hosting.get(&id) {
                return Some(ModalCall {
                    window: hosted.state.hwnd(),
                    modal,
                    maximized: hosted.state.is_maximized(),
                    resizable: hosted.state.is_resizable(),
                });
            }
        }
        None
    }

    pub(crate) fn take_drag_loop(&mut self) -> Option<DragLoop> {
        self.dragging.take_loop()
    }

    pub(crate) fn drag_finished(&mut self, result: HRESULT, effect: DROPEFFECT) {
        let Self {
            hosting,
            dragging,
            touched,
            ..
        } = self;
        dragging.finished(&mut Windows::new(hosting, touched), result, effect);
    }

    pub(crate) fn drag_step(&mut self) {
        self.dragging.step(&self.hosting);
        self.modal_turn();
    }

    pub(crate) fn drag_entered(
        &mut self,
        window: WindowId,
        data: IDataObject,
        screen: (i32, i32),
        allowed: DROPEFFECT,
    ) -> DROPEFFECT {
        let Self {
            hosting,
            dragging,
            touched,
            ..
        } = self;
        dragging.entered(
            &mut Windows::new(hosting, touched),
            window,
            data,
            screen,
            allowed,
        )
    }

    pub(crate) fn drag_moved(
        &mut self,
        window: WindowId,
        screen: (i32, i32),
        allowed: DROPEFFECT,
    ) -> DROPEFFECT {
        let Self {
            hosting,
            dragging,
            touched,
            ..
        } = self;
        dragging.moved(&mut Windows::new(hosting, touched), window, screen, allowed)
    }

    pub(crate) fn drag_left(&mut self, window: WindowId) {
        let Self {
            hosting,
            dragging,
            touched,
            ..
        } = self;
        dragging.left(&mut Windows::new(hosting, touched), window);
    }

    pub(crate) fn drag_dropped(
        &mut self,
        window: WindowId,
        _data: IDataObject,
        screen: (i32, i32),
        allowed: DROPEFFECT,
    ) -> DROPEFFECT {
        let Self {
            hosting,
            dragging,
            touched,
            ..
        } = self;
        dragging.dropped(&mut Windows::new(hosting, touched), window, screen, allowed)
    }

    pub(crate) fn modal_turn(&mut self) {
        if self.exiting {
            return;
        }
        self.turn();
    }

    pub(crate) fn turn(&mut self) -> Option<Instant> {
        while let Some(event) = self.mailbox.take() {
            self.user_event(event);
        }
        self.hear();
        let now = Instant::now();
        for hosted in self.hosting.values_mut() {
            let id = hosted.state.id;
            if let Ok(mut input) = hosted.state.shared().input.try_borrow_mut() {
                input.mouse.settle_scroll(id, now);
            }
        }
        self.hear();
        if self.clock.take_tick() {
            for hosted in self.hosting.values_mut() {
                if hosted.awaits_frame() {
                    hosted.frame_answered();
                }
            }
        }
        self.settle_windows();
        let mut touched = std::mem::take(&mut self.touched);
        for id in touched.drain(..) {
            self.service(id);
        }
        touched.append(&mut self.touched);
        self.touched = touched;
        {
            let Self {
                hosting,
                dragging,
                touched,
                ..
            } = self;
            dragging.settle_within(&mut Windows::new(hosting, touched));
        }
        let openings = window::take_requested_windows();
        self.open_requested(openings);
        let outcome = self.hosting.frame(|_| {});
        self.after_frame();
        let carried = self.dragging.paint_ghost(&mut self.hosting);
        self.service_all();
        self.follow_pointers();
        if self.hosting.iter().any(|(_, hosted)| hosted.awaits_frame()) {
            self.clock.arm();
        }
        let hinted = outcome.wake.delay().map(|delay| Instant::now() + delay);
        let woken = hosting::next_wake(&self.hosting);
        let scrolling = self
            .hosting
            .iter()
            .filter_map(|(_, hosted)| {
                hosted
                    .state
                    .shared()
                    .input
                    .try_borrow()
                    .ok()
                    .and_then(|input| input.mouse.scroll_due())
            })
            .min();
        hinted
            .into_iter()
            .chain(woken)
            .chain(carried)
            .chain(scrolling)
            .min()
    }

    fn user_event(&mut self, event: LoopEvent) {
        match event {
            LoopEvent::Opened(open) => self.opened(*open),
            LoopEvent::Message(message) => {
                self.hosting.deliver(&message.window, message.inbound);
                self.touch(message.window);
            }
        }
    }

    fn touch(&mut self, id: WindowId) {
        if !self.touched.contains(&id) {
            self.touched.push(id);
        }
    }

    fn hear(&mut self) {
        let mut heard = std::mem::take(&mut self.heard);
        heard::take(&mut heard);
        let mut appearance = false;
        let mut displays = false;
        for (id, event) in heard.drain(..) {
            match event {
                Heard::Input { event, stylus } => self.heard_input(id, event, stylus),
                Heard::Focus(entered) => self.focused(id, entered),
                Heard::Resized | Heard::Rescaled => self.touch(id),
                Heard::CloseRequested => {
                    if let Some(hosted) = self.hosting.get_mut(&id) {
                        hosted.state.close_requested = true;
                    }
                }
                Heard::Appearance => appearance = true,
                Heard::Displays => displays = true,
            }
        }
        self.heard = heard;
        if appearance {
            self.follow_appearance();
        }
        if displays {
            self.publish_refresh();
        }
    }

    fn heard_input(&mut self, id: WindowId, event: Event, stylus: Option<Stylus>) {
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        if matches!(event, Event::PointerTravelled(_)) && !hosted.state.is_locked() {
            return;
        }
        hosted.ui.handle_input(event, stylus);
        self.touch(id);
    }

    fn focused(&mut self, id: WindowId, entered: bool) {
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        if let Some(hook) = hosted.state.keyboard(entered) {
            hosted.ui.start(hook);
        }
        self.touch(id);
    }

    fn follow_appearance(&mut self) {
        let current = Appearance::current();
        let theme = current.theme();
        let following = self.appearance.is_some();
        if following && self.appearance.as_ref() != Some(&current) {
            self.appearance = Some(current);
            self.retheme(theme);
        }
        let ids: Vec<WindowId> = self.hosting.iter().map(|(id, _)| *id).collect();
        for id in ids {
            self.follow_look(id);
        }
    }

    fn retheme(&mut self, theme: Theme) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        for hosted in self.hosting.values_mut() {
            hosted.ui.set_theme(self.theme.clone());
        }
    }

    fn follow_look(&mut self, id: WindowId) {
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        let look = Look::of(hosted.ui.theme());
        hosted.state.follow_look(look);
    }

    fn opened(&mut self, open: LaunchRequest) {
        if matches!(self.policy, Some(Policy::Single)) {
            if let Some((_, hosted)) = self.hosting.iter().next() {
                bring_forward(hosted.state.hwnd());
            }
            let reached = self.hosting.values_mut().find_map(|hosted| {
                let handler = hosted.state.opened(open.clone())?;
                hosted.ui.start(handler);
                Some(hosted.state.id)
            });
            if let Some(id) = reached {
                self.touch(id);
            }
            return;
        }
        let Some(Policy::Windows(make)) = &self.policy else {
            return;
        };
        let windows = make(open);
        self.open_requested(windows);
    }

    fn open_pending(&mut self) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let app_name = self.app_name.clone();
        let device = self.device;
        self.hosting
            .renderer_or_try_init(|| {
                let gpu = device
                    .ask(rikka_gpu::Gpu::builder().app_name(app_name))
                    .build()?;
                Ok::<_, Error>(Renderer::new(&gpu)?)
            })
            .map(drop)?;
        let pointer = self.cursors.pointer(CursorShape::Default);
        for opening in std::mem::take(&mut self.pending) {
            let id = self.next_id;
            self.next_id = id.after();
            let opener = Opener {
                class: self.class,
                proxy: &self.proxy,
                theme: &self.theme,
                pointer,
            };
            open::open(&mut self.hosting, id, &opener, opening)?;
            self.resize(id);
            let Self {
                hosting,
                dragging,
                touched,
                ..
            } = self;
            dragging.opened(&mut Windows::new(hosting, touched), id);
            self.touch(id);
        }
        self.publish_refresh();
        Ok(())
    }

    fn open_requested(&mut self, openings: Vec<Opening>) {
        if openings.is_empty() {
            return;
        }
        self.pending.extend(openings);
        if let Err(error) = self.open_pending() {
            self.pending.clear();
            tracing::error!("failed to open window: {error}");
        }
    }

    fn settle_windows(&mut self) {
        let ids: Vec<WindowId> = self.hosting.iter().map(|(id, _)| *id).collect();
        let mut moved = false;
        for id in ids {
            let Some(hosted) = self.hosting.get_mut(&id) else {
                continue;
            };
            let display = hosted.state.display;
            let changed = hosted.state.measure();
            moved |= display != hosted.state.display;
            if changed {
                self.resize(id);
            } else if let Some(hosted) = self.hosting.get_mut(&id) {
                let drawable = hosted.state.is_drawable() && hosted.has_surface();
                if drawable != hosted.is_drawable() {
                    if drawable {
                        hosted.frame_answered();
                        hosted.ui.request_redraw();
                    }
                    hosted.drawable(drawable);
                }
            }
            let Some(hosted) = self.hosting.get_mut(&id) else {
                continue;
            };
            if std::mem::take(&mut hosted.state.close_requested) {
                match hosted.state.closing.ask(&mut hosted.ui) {
                    true => self.close(id),
                    false => self.touch(id),
                }
            }
        }
        if moved {
            self.publish_refresh();
        }
    }

    fn resize(&mut self, id: WindowId) {
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        let Some(measured) = hosted.state.measured() else {
            return;
        };
        let size = (measured.pixels.width, measured.pixels.height);
        let scale = measured.scale;
        let logical: Size = measured.logical();
        hosted.ui.set_viewport(logical, scale as f32);
        if hosted.has_surface() {
            hosted.adjust(move |presenter| presenter.resize(size));
            hosted.ui.request_redraw();
        } else if let Err(error) = self.attach_presenter(id, size) {
            self.fail(error);
            return;
        }
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        let drawable = hosted.state.is_drawable();
        if drawable {
            hosted.frame_answered();
            hosted.ui.request_redraw();
        }
        hosted.drawable(drawable);
        self.touch(id);
    }

    fn attach_presenter(&mut self, id: WindowId, size: (u32, u32)) -> Result<()> {
        let Some(hosted) = self.hosting.get(&id) else {
            return Ok(());
        };
        let window = hosted.state.hwnd();
        let mut presenter = {
            let mut renderer = self.hosting.renderer().expect("renderer not initialized");
            Presenter::new(window.0 as isize, &mut renderer, SurfaceConfig::default())?
        };
        presenter.resize(size);
        if let Some(hosted) = self.hosting.get_mut(&id) {
            hosted.attach(presenter);
        }
        Ok(())
    }

    fn service(&mut self, id: WindowId) {
        {
            let Self {
                hosting,
                dragging,
                touched,
                ..
            } = self;
            dragging.begin_from(&mut Windows::new(hosting, touched), id);
        }
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        let closes = hosted.state.service(&mut hosted.ui);
        let raising = hosted.state.take_raising();
        let modals = hosted.state.take_modals();
        let quits = hosted.state.take_quitting();
        self.modals
            .extend(modals.into_iter().map(|modal| (id, modal)));
        self.raise(raising);
        if closes {
            self.close(id);
        }
        if quits {
            self.quit();
        }
    }

    fn service_all(&mut self) {
        let ids: Vec<WindowId> = self.hosting.iter().map(|(id, _)| *id).collect();
        for id in ids {
            self.follow_look(id);
            let Some(hosted) = self.hosting.get_mut(&id) else {
                continue;
            };
            let closes = hosted.state.service(&mut hosted.ui);
            let raising = hosted.state.take_raising();
            let modals = hosted.state.take_modals();
            let quits = hosted.state.take_quitting();
            self.modals
                .extend(modals.into_iter().map(|modal| (id, modal)));
            self.raise(raising);
            if closes {
                self.close(id);
            }
            if quits {
                self.quit();
            }
        }
    }

    fn quit(&mut self) {
        let ids: Vec<WindowId> = self.hosting.iter().map(|(id, _)| *id).collect();
        for id in ids {
            let Some(hosted) = self.hosting.get_mut(&id) else {
                continue;
            };
            let closes =
                hosted.state.closing.ask(&mut hosted.ui) || hosted.state.service(&mut hosted.ui);
            if closes {
                self.close(id);
            }
        }
    }

    fn raise(&self, targets: Vec<WindowRef>) {
        for target in targets {
            match self.hosting.get(&WindowId::from(target.get())) {
                Some(raised) => bring_forward(raised.state.hwnd()),
                None => tracing::debug!("no window matches {target:?}"),
            }
        }
    }

    fn close(&mut self, id: WindowId) {
        if let Some(hosted) = self.hosting.get(&id) {
            hosted.state.alive.close();
        }
        self.dragging.window_gone(id);
        self.modals.retain(|(window, _)| *window != id);
        drop(self.hosting.remove(&id));
        if self.hosting.is_empty() {
            self.exiting = true;
        }
    }

    fn after_frame(&mut self) {
        for hosted in self.hosting.values_mut() {
            let presented = std::sync::Arc::clone(&hosted.state.presented);
            if hosted.has_surface() && !hosted.is_painting() {
                hosted.adjust(move |presenter| {
                    if let Some(frame) = presenter.take_presented()
                        && let Ok(mut slot) = presented.lock()
                    {
                        *slot = Some(frame);
                    }
                });
            }
            let frame = hosted
                .state
                .presented
                .lock()
                .ok()
                .and_then(|mut slot| slot.take());
            if let Some(frame) = frame {
                hosted.presented(frame.at, frame.refresh);
            }
            if hosted.state.take_first_show() {
                placement::show(hosted.state.hwnd(), SW_SHOWNORMAL);
            }
        }
    }

    fn follow_pointers(&mut self) {
        let mut shown = Vec::new();
        for hosted in self.hosting.values_mut() {
            let shape = hosted.ui.cursor();
            shown.push(shape);
            let pointer = self.cursors.pointer(shape);
            let shared = std::rc::Rc::clone(hosted.state.shared());
            let caret = match hosted.state.is_focused() {
                true => hosted.ui.caret(),
                false => None,
            };
            shared.caret.set(caret);
            let Ok(mut input) = shared.input.try_borrow_mut() else {
                continue;
            };
            if hosted.state.is_focused() {
                input.ime.follow(hosted.state.hwnd(), caret);
            }
            if shared.pointer.replace(pointer) != pointer && input.mouse.hovers_client() {
                show_pointer(pointer);
            }
        }
        self.cursors.prune(&shown);
    }

    fn publish_refresh(&self) {
        monitor::publish(self.hosting.iter().map(|(_, hosted)| hosted.state.display));
    }
}

impl std::fmt::Debug for Runner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner")
            .field("app_name", &self.app_name)
            .field("dragging", &self.dragging)
            .finish_non_exhaustive()
    }
}
