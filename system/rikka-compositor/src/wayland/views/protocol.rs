use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_protocols::rikka_window_view_manager_v1::{
    self, Error as ManagerError, RikkaWindowViewManagerV1,
};
use rikka_compositor_protocols::rikka_window_view_v1::{self, Error, RikkaWindowViewV1};
use rikka_compositor_server::{DisplayHandle, Module};
use rikka_types::{CornerRadii, Rect as SceneRect, RoundedRect};

use super::registry::{Look, ViewId};
use crate::context::windows_ref;
use crate::state::Compositor;

pub(crate) struct WindowViewProtocol;

impl Module<Compositor> for WindowViewProtocol {
    fn install(self, display: &DisplayHandle<Compositor>) {
        display.serve::<RikkaWindowViewManagerV1>(manager);
        display
            .serve::<RikkaWindowViewV1>(view)
            .destroyed(view_destroyed);
        display
            .global::<RikkaWindowViewManagerV1>()
            .privileged()
            .publish();
    }
}

fn manager(
    compositor: &mut Compositor,
    manager: RikkaWindowViewManagerV1,
    request: rikka_window_view_manager_v1::Request<'_>,
) {
    let rikka_window_view_manager_v1::Request::GetWindowView {
        id,
        surface,
        window,
    } = request
    else {
        return;
    };
    let Some(surface) = compositor.surfaces.lookup(&surface) else {
        return;
    };
    if compositor.surfaces.root(surface) != surface {
        return manager.post_error(ManagerError::NotARoot);
    }
    let window = windows_ref!(compositor).window_called(window);
    let view = compositor.views.open(surface, window);
    id.set_key(view);
}

fn view(
    compositor: &mut Compositor,
    view: RikkaWindowViewV1,
    request: rikka_window_view_v1::Request,
) {
    let Some(id) = view.key::<ViewId>() else {
        return;
    };
    let change: Box<dyn FnOnce(&mut Look)> = match request {
        rikka_window_view_v1::Request::Destroy => return,
        rikka_window_view_v1::Request::SetDestination {
            x,
            y,
            width,
            height,
        } => {
            let Some(destination) = destination(x, y, width, height) else {
                return view.post_error(Error::InvalidDestination);
            };
            Box::new(move |look| look.destination = Some(destination))
        }
        rikka_window_view_v1::Request::SetClip {
            x,
            y,
            width,
            height,
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        } => {
            let Some(clip) = clip(
                [x, y, width, height],
                [top_left, top_right, bottom_right, bottom_left],
            ) else {
                return view.post_error(Error::InvalidClip);
            };
            Box::new(move |look| look.clip = clip)
        }
        rikka_window_view_v1::Request::SetOpacity { opacity } => {
            if !(0.0..=1.0).contains(&opacity) {
                return view.post_error(Error::InvalidOpacity);
            }
            Box::new(move |look| look.opacity = opacity as f32)
        }
        rikka_window_view_v1::Request::SetOrder { order } => {
            Box::new(move |look| look.order = order)
        }
    };
    compositor.views.change(id, change);
}

fn view_destroyed(compositor: &mut Compositor, view: RikkaWindowViewV1) {
    if let Some(id) = view.key::<ViewId>() {
        compositor.views.close(id);
    }
}

fn destination(x: f64, y: f64, width: f64, height: f64) -> Option<Rect<f64, Logical>> {
    let finite = [x, y, width, height].iter().all(|value| value.is_finite());
    (finite && width > 0.0 && height > 0.0)
        .then(|| Rect::new(Point::new(x, y), Size::new(width, height)))
}

fn clip(rect: [f64; 4], radii: [f64; 4]) -> Option<Option<RoundedRect>> {
    let [x, y, width, height] = rect;
    let finite = rect.iter().chain(&radii).all(|value| value.is_finite());
    if !finite || width < 0.0 || height < 0.0 || radii.iter().any(|radius| *radius < 0.0) {
        return None;
    }
    if width == 0.0 && height == 0.0 {
        return Some(None);
    }
    let [top_left, top_right, bottom_right, bottom_left] = radii.map(|radius| radius as f32);
    Some(Some(RoundedRect::new(
        SceneRect::new(x as f32, y as f32, width as f32, height as f32),
        CornerRadii::new(top_left, top_right, bottom_right, bottom_left),
    )))
}
