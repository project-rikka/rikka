use std::cell::OnceCell;
use std::rc::Rc;
use std::sync::Arc;

use ash::vk::{self, Handle};
use rikka_app::Sender;
use rikka_app::embedded::{Embedded, Embedding, SurfaceId};
use rikka_renderer::{ExternalImage, Transform};
use rikka_ui::{Event, Handler, Size};

use crate::abi::violation;
use crate::error;
use crate::ffi;
use crate::handlers::{Callback, action};
use crate::input;
use crate::window::{Model, Window};

#[derive(Debug)]
pub(crate) struct EmbeddedSurfaceBuilder {
    view: Callback<ffi::ViewFn>,
    start: Option<Handler>,
    close: Option<Handler>,
}

impl EmbeddedSurfaceBuilder {
    pub(crate) fn new(view: Callback<ffi::ViewFn>) -> Self {
        Self {
            view,
            start: None,
            close: None,
        }
    }

    pub(crate) fn on_start(&mut self, callback: Callback<ffi::ActionFn>) {
        self.start = Some(action(callback));
    }

    pub(crate) fn on_close_request(&mut self, callback: Callback<ffi::ActionFn>) {
        self.close = Some(action(callback));
    }

    pub(crate) fn open(self, embedding: &Rc<Embedding>) -> EmbeddedSurface {
        let Self { view, start, close } = self;
        let window = Rc::new(OnceCell::new());
        let opened = Rc::clone(&window);
        let mut surface = Embedded::new(
            move |sender: Sender<Model>| {
                let model = Model::new(sender, String::new());
                opened.get_or_init(|| Arc::clone(model.window()));
                model
            },
            move |model: &Model| model.render(&view),
        );
        if let Some(start) = start {
            surface = surface.on_start(start);
        }
        if let Some(close) = close {
            surface = surface.on_close_request(close);
        }
        let id = embedding.open(surface);
        let window = window
            .get()
            .cloned()
            .expect("an embedded surface makes its model as it opens");
        EmbeddedSurface {
            embedding: Rc::clone(embedding),
            id,
            window,
        }
    }
}

#[derive(Debug)]
pub(crate) struct EmbeddedSurface {
    embedding: Rc<Embedding>,
    id: SurfaceId,
    window: Arc<Window>,
}

impl EmbeddedSurface {
    pub(crate) fn window(&self) -> Arc<Window> {
        Arc::clone(&self.window)
    }

    pub(crate) fn resize(&self, width: f32, height: f32, scale: f32) {
        let valid = [width, height]
            .iter()
            .all(|side| side.is_finite() && *side >= 0.0)
            && scale.is_finite()
            && scale > 0.0;
        if !valid {
            violation(format!(
                "{width} x {height} at scale {scale} is not a surface size"
            ));
        }
        self.embedding
            .resize(self.id, Size::new(width, height), scale);
    }

    pub(crate) fn render(&self, target: ffi::VulkanImage) -> ffi::Status {
        let image = external_image(target);
        match unsafe { self.embedding.draw(self.id, image) } {
            Ok(()) => ffi::Status::Ok,
            Err(failure) => error::fail(error::app_status(&failure), failure),
        }
    }

    pub(crate) fn handle(&self, event: Event) -> bool {
        self.embedding.handle_event(self.id, event)
    }

    pub(crate) fn cursor(&self) -> ffi::CursorShape {
        input::cursor(self.embedding.cursor(self.id))
    }

    pub(crate) fn caret(&self) -> ffi::Rect {
        let caret = self.embedding.caret(self.id).unwrap_or_default();
        ffi::Rect {
            x: caret.x(),
            y: caret.y(),
            width: caret.width(),
            height: caret.height(),
        }
    }
}

impl Drop for EmbeddedSurface {
    fn drop(&mut self) {
        self.embedding.close(self.id);
    }
}

fn external_image(target: ffi::VulkanImage) -> ExternalImage {
    let ffi::VulkanImage {
        image,
        view,
        format,
        width,
        height,
        layout,
        bottom_up,
    } = target;
    if image == 0 || view == 0 {
        violation("the target image or its view is NULL");
    }
    if width == 0 || height == 0 {
        violation(format!("a {width} x {height} target is empty"));
    }
    let raw = |value: u32, what: &str| {
        i32::try_from(value).unwrap_or_else(|_| violation(format!("{value} is not a {what}")))
    };
    ExternalImage {
        image: vk::Image::from_raw(image),
        view: vk::ImageView::from_raw(view),
        format: vk::Format::from_raw(raw(format, "VkFormat")),
        width,
        height,
        layout: vk::ImageLayout::from_raw(raw(layout, "VkImageLayout")),
        transform: match bottom_up {
            true => Transform::Flipped180,
            false => Transform::Normal,
        },
    }
}
