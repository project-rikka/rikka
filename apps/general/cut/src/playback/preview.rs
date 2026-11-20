use rikka_sync::Mutex;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use cut_project::MediaId;
use cut_render::{RenderedFrame, Renderer, Request};
use rikka_image_edit::Imaging;
use rikka_picture::SharedGpu;
use rikka_ui::{ComputedImage, ImageSource, NodeHandle};

#[derive(Clone, Default)]
pub(crate) struct Preview(Rc<Inner>);

#[derive(Default)]
struct Inner {
    renderer: RefCell<Option<Renderer>>,
    refused: Cell<bool>,
    awaiting_gpu: Cell<bool>,
    media_round: Cell<u64>,
    outlet: Arc<FrameSink>,
}

#[derive(Default)]
struct FrameSink {
    computed: ComputedImage,
    offline: Mutex<Arc<BTreeSet<MediaId>>>,
    watcher: Mutex<Option<NodeHandle>>,
}

impl FrameSink {
    fn publish(&self, frame: &RenderedFrame) {
        self.computed.show(frame.picture.clone());
        let mut offline = self.offline.lock();
        if *offline == frame.offline {
            return;
        }
        *offline = Arc::clone(&frame.offline);
        drop(offline);
        self.repaint_watcher();
    }

    fn repaint_watcher(&self) {
        if let Some(watcher) = self.watcher.lock().as_ref() {
            watcher.repaint();
        }
    }
}

impl Preview {
    pub(crate) fn same(&self, other: &Preview) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn submit(&self, request: Request) {
        if self.0.refused.get() {
            return;
        }
        let mut renderer = self.0.renderer.borrow_mut();
        if renderer.is_none() {
            let Some(compute) = SharedGpu::compute(Duration::ZERO) else {
                self.repaint_when_gpu_ready();
                return;
            };
            let imaging = Imaging::shared(&compute);
            let outlet = Arc::clone(&self.0.outlet);
            match Renderer::start(imaging, move |frame| outlet.publish(frame)) {
                Ok(started) => *renderer = Some(started),
                Err(error) => {
                    tracing::warn!(%error, "cannot start preview renderer");
                    self.0.refused.set(true);
                    return;
                }
            }
        }
        if let Some(renderer) = renderer.as_ref() {
            renderer.submit(request);
        }
    }

    fn repaint_when_gpu_ready(&self) {
        if self.0.awaiting_gpu.replace(true) {
            return;
        }
        let outlet = Arc::clone(&self.0.outlet);
        SharedGpu::on_compute(move |_| outlet.repaint_watcher());
    }

    pub(crate) fn picture(&self) -> Option<ImageSource> {
        let computed = &self.0.outlet.computed;
        computed.size()?;
        Some(ImageSource::Computed(computed.clone()))
    }

    pub(crate) fn offline(&self) -> Arc<BTreeSet<MediaId>> {
        Arc::clone(&self.0.outlet.offline.lock())
    }

    pub(crate) fn media_round(&self) -> u64 {
        self.0.media_round.get()
    }

    pub(crate) fn reopen_offline(&self) {
        self.0.media_round.set(self.0.media_round.get() + 1);
        self.0.outlet.repaint_watcher();
    }

    pub(crate) fn watch(&self, handle: NodeHandle) {
        *self.0.outlet.watcher.lock() = Some(handle);
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.0
            .renderer
            .borrow()
            .as_ref()
            .is_none_or(Renderer::is_current)
    }
}

impl std::fmt::Debug for Preview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preview")
            .field("renderer", &self.0.renderer.borrow())
            .finish_non_exhaustive()
    }
}
