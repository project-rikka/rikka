use cut_project::{Appearance, Camera, ClipId, Format, Redraw, Viewpoint};
use rikka_app::prelude::*;
use rikka_figure::{GRIP_REACH, TURN_GAP};
use rikka_figure_editing::{paint_line, paint_loop};
use rikka_ui::DRAG_SLOP;

use super::Stage;
use super::viewing::Viewing;
use crate::tool::Tool;

const MARGIN: f32 = 0.85;
const MIN_SCALE: f32 = 0.02;
const ORBIT: f32 = 0.3;
const DOLLY_PER_NOTCH: f32 = 0.9;
const NEAREST: f32 = 0.02;
const HANDLE: f32 = 8.0;
const KNOB: f32 = 9.0;
const KNOB_CORE: f32 = 5.0;
const ARM: f32 = 7.0;
const LINE: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CameraGesture {
    Pan,
    Zoom,
    Roll,
    Orbit,
}

#[derive(Clone, Debug)]
pub(crate) struct Framing {
    camera: ClipId,
    scale: f32,
    lens: Camera,
    quad: [Point; 4],
    interest: Point,
    format: Format,
}

#[derive(Clone, Debug)]
pub(crate) struct Reframing {
    camera: ClipId,
    scale: f32,
    gesture: CameraGesture,
    lens: Camera,
    interest: Point,
    from: Point,
    origin: Point,
    moved: bool,
    framed: Camera,
}

impl Stage {
    pub(super) fn filmed(&self) -> Option<(ClipId, Camera)> {
        let [id] = self.selected.iter().copied().collect::<Vec<_>>()[..] else {
            return None;
        };
        if self.tool != Tool::Select {
            return None;
        }
        let project = self.shown_project();
        let frame = self.transport.frame();
        let (layer, clip) = project.locate(id)?;
        let visible = project
            .layer(layer)
            .is_some_and(|layer_ref| layer_ref.is_shown());
        (visible && clip.covers(frame))
            .then(|| clip.camera_at(frame))
            .flatten()
            .map(|camera| (id, camera))
    }

    pub(super) fn framing(&self, scale: Option<f32>) -> Option<Framing> {
        let (camera, lens) = self.filmed()?;
        let project = self.shown_project();
        let frame = self.transport.frame();
        let format = project.format();
        let view = project.camera_view(camera, frame, 1.0)?;
        let scale = scale.unwrap_or_else(|| fitted(&view, format));
        let middle = format.center();
        let quad = view.map(|corner| middle + (corner - middle) * scale);
        Some(Framing {
            camera,
            scale,
            lens,
            quad,
            interest: format.center() + lens.interest * scale,
            format,
        })
    }

    pub(super) fn view(&self) -> Viewpoint {
        match self.framing(None) {
            Some(framing) => framing.viewpoint(),
            None => Viewpoint::Output,
        }
    }
}

impl Framing {
    pub(crate) fn viewpoint(&self) -> Viewpoint {
        Viewpoint::Overview {
            camera: self.camera,
            scale: self.scale,
        }
    }

    fn knob(&self, viewing: &Viewing) -> (Point, Point) {
        let [first, second, ..] = self.quad.map(|corner| viewing.to_screen(corner));
        let middle = (first + second) * 0.5;
        let along = second - first;
        let length = along.x.hypot(along.y).max(f32::EPSILON);
        let mut outward = Point::new(along.y, -along.x) * (1.0 / length);
        let inside = viewing.to_screen(self.interest) - middle;
        if outward.x * inside.x + outward.y * inside.y > 0.0 {
            outward = -outward;
        }
        (middle, middle + outward * TURN_GAP)
    }

    pub(crate) fn contains(&self, at: Point) -> bool {
        let sides: Vec<f32> = (0..4)
            .map(|index| {
                let (from, to) = (self.quad[index], self.quad[(index + 1) % 4]);
                let (along, direction) = (to - from, at - from);
                along.x * direction.y - along.y * direction.x
            })
            .collect();
        sides.iter().all(|side| *side >= 0.0) || sides.iter().all(|side| *side <= 0.0)
    }

    pub(crate) fn hit(
        &self,
        position: Point,
        viewing: &Viewing,
        alt: bool,
    ) -> Option<CameraGesture> {
        let near = |spot: Point| spot.distance(position) <= GRIP_REACH;
        if near(self.knob(viewing).1) {
            return Some(CameraGesture::Roll);
        }
        if self
            .quad
            .iter()
            .any(|corner| near(viewing.to_screen(*corner)))
        {
            return Some(CameraGesture::Zoom);
        }
        self.contains(viewing.to_stage(position))
            .then_some(match alt {
                true => CameraGesture::Orbit,
                false => CameraGesture::Pan,
            })
    }

    pub(crate) fn cursor(&self, position: Point, viewing: &Viewing) -> Option<CursorShape> {
        Some(match self.hit(position, viewing, false)? {
            CameraGesture::Roll => CursorShape::Grab,
            CameraGesture::Zoom => CursorShape::Crosshair,
            CameraGesture::Pan | CameraGesture::Orbit => CursorShape::Move,
        })
    }

    pub(crate) fn reframe(&self, gesture: CameraGesture, at: Point, origin: Point) -> Reframing {
        Reframing {
            camera: self.camera,
            scale: self.scale,
            gesture,
            lens: self.lens,
            interest: self.interest,
            from: at,
            origin,
            moved: false,
            framed: self.lens,
        }
    }

    pub(crate) fn dollied(&self, notches: f32) -> (ClipId, Camera) {
        let nearest = self.lens.focal(self.format) * f64::from(NEAREST);
        let factor = f64::from(DOLLY_PER_NOTCH.powf(notches));
        let lens = self
            .lens
            .dollied(|distance| (distance * factor).max(nearest), self.format);
        (self.camera, lens)
    }

    pub(crate) fn paint(&self, ctx: &mut PaintCtx<'_>, viewing: &Viewing) {
        let accent = palette().accent;
        let corners = self.quad.map(|corner| viewing.to_screen(corner));
        paint_loop(ctx, &corners, accent);
        let (middle, knob) = self.knob(viewing);
        paint_line(ctx, middle, knob, accent);
        ctx.draw_circle(knob, KNOB, accent);
        ctx.draw_circle(knob, KNOB_CORE, palette().surface);
        for corner in corners {
            ctx.draw_quad(
                Quad::new(Rect::from_center(corner, Size::new(HANDLE, HANDLE)))
                    .fill(palette().surface)
                    .stroke(LINE, accent),
            );
        }
        let interest = viewing.to_screen(self.interest);
        paint_line(
            ctx,
            interest - Point::new(ARM, 0.0),
            interest + Point::new(ARM, 0.0),
            accent,
        );
        paint_line(
            ctx,
            interest - Point::new(0.0, ARM),
            interest + Point::new(0.0, ARM),
            accent,
        );
    }
}

impl Reframing {
    pub(crate) fn viewpoint(&self) -> Viewpoint {
        Viewpoint::Overview {
            camera: self.camera,
            scale: self.scale,
        }
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.moved
    }

    pub(crate) fn drag(&mut self, at: Point, position: Point) -> bool {
        if !self.moved && self.origin.distance(position) < DRAG_SLOP {
            return false;
        }
        self.moved = true;
        let lens = self.lens;
        self.framed = match self.gesture {
            CameraGesture::Pan => Camera {
                interest: lens.interest + (at - self.from) * (1.0 / self.scale),
                ..lens
            },
            CameraGesture::Zoom => {
                let reach = |point: Point| point.distance(self.interest).max(f32::EPSILON);
                Camera {
                    zoom: lens.zoom * reach(self.from) / reach(at),
                    ..lens
                }
            }
            CameraGesture::Roll => {
                let turn = |point: Point| {
                    let offset = point - self.interest;
                    offset.y.atan2(offset.x).to_degrees()
                };
                Camera {
                    roll: lens.roll + turn(at) - turn(self.from),
                    ..lens
                }
            }
            CameraGesture::Orbit => {
                let by = position - self.origin;
                Camera {
                    pan: lens.pan + by.x * ORBIT,
                    tilt: lens.tilt + by.y * ORBIT,
                    ..lens
                }
            }
        }
        .checked();
        true
    }

    pub(crate) fn changes(&self) -> Vec<(ClipId, Appearance)> {
        vec![(self.camera, Appearance::Camera(self.framed))]
    }

    pub(crate) fn redraw(&self) -> Redraw {
        match self.gesture {
            CameraGesture::Pan => Redraw::Move,
            CameraGesture::Zoom => Redraw::Resize,
            CameraGesture::Roll | CameraGesture::Orbit => Redraw::Rotate,
        }
    }
}

fn fitted(quad: &[Point; 4], format: Format) -> f32 {
    let middle = format.center();
    quad.iter()
        .flat_map(|corner| {
            let reach = *corner - middle;
            [
                middle.x * MARGIN / reach.x.abs().max(f32::EPSILON),
                middle.y * MARGIN / reach.y.abs().max(f32::EPSILON),
            ]
        })
        .fold(1.0f32, f32::min)
        .max(MIN_SCALE)
}
