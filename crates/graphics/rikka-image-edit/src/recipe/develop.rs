use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use crate::error::{Error, Result};
use crate::ops::Reach;
use crate::ops::passes::CopyPush;
use crate::picture::{Frame, Mode, Picture};
use crate::plane::{PlaneRef, saturating_i32};
use crate::recipe::mask::grown;
use crate::recipe::{Recipe, Step};
use crate::rewrite::rewritable;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const COPY: u32 = 0;
const GRAY: u32 = 2;
const PASTE: u32 = 4;

const ALIGNMENT: u32 = 16;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlendPush {
    head: PushHeader,
    top: PlaneRef,
    mask: PlaneRef,
    opacity: f32,
    masked: u32,
    seed: u32,
    pad: u32,
}

pub(crate) fn develop(picture: &Picture, recipe: &Recipe) -> Result<Picture> {
    if recipe.is_empty() {
        return Ok(picture.clone());
    }
    let canvas = picture.frame().extent();
    if let Reach::Within { area, halo } = recipe.reach(canvas)
        && let Some((origin, extent)) = window(grown(area, halo, canvas), canvas)
    {
        return develop_window(picture, recipe, origin, extent, area);
    }
    let mut work = Work::new(picture.imaging());
    let mut frame = picture.frame().clone();
    for (index, step) in recipe.steps().iter().enumerate() {
        frame = apply(&mut work, &frame, step, 1.0, index as u32)?;
    }
    let stored = store(&mut work, &frame)?;
    work.finish()?;
    Ok(Picture::from_frame(picture.imaging(), stored))
}

fn window(area: [u32; 4], canvas: [u32; 2]) -> Option<([i32; 2], [u32; 2])> {
    let [x, y, width, height] = area;
    let left = x / ALIGNMENT * ALIGNMENT;
    let top = y / ALIGNMENT * ALIGNMENT;
    let right = (x + width).next_multiple_of(ALIGNMENT).min(canvas[0]);
    let bottom = (y + height).next_multiple_of(ALIGNMENT).min(canvas[1]);
    let extent = [right.checked_sub(left)?, bottom.checked_sub(top)?];
    let windowed = u64::from(extent[0]) * u64::from(extent[1]);
    let whole = u64::from(canvas[0]) * u64::from(canvas[1]);
    let worthwhile = extent[0] > 0 && extent[1] > 0 && windowed * 2 <= whole;
    let origin = [i32::try_from(left).ok()?, i32::try_from(top).ok()?];
    worthwhile.then_some((origin, extent))
}

fn develop_window(
    picture: &Picture,
    recipe: &Recipe,
    origin: [i32; 2],
    extent: [u32; 2],
    changed: [u32; 4],
) -> Result<Picture> {
    let source = picture.frame();
    let mut work = Work::windowed_on(picture.imaging(), source.extent(), origin);
    let input = work.plane(extent, source.plane.format())?;
    let copied = CopyPush {
        head: PushHeader::new(&source.plane, &input).mode(COPY),
        from: origin,
        quarter: 0,
        mirror: 0,
        fill: [0.0; 4],
    };
    work.pass(Shader::Copy, copied, Io::new(&source.plane, &input), 1.0)?;
    let mut frame = source.with_plane(input);
    for (index, step) in recipe.steps().iter().enumerate() {
        frame = apply(&mut work, &frame, step, 1.0, index as u32)?;
    }
    let stored = store(&mut work, &frame)?;
    let [left, top, across, down] = changed;
    let [width, height] = source.extent();
    let paste_area = [
        left,
        top,
        across.next_multiple_of(ALIGNMENT).min(width - left),
        down.next_multiple_of(ALIGNMENT).min(height - top),
    ];
    let format = stored.plane.format();
    let out = rewritable(&mut work, &source.plane, format, paste_area)?;
    let at = [saturating_i32(left), saturating_i32(top)];
    let pasted = CopyPush {
        head: PushHeader::new(&stored.plane, &out)
            .mode(PASTE)
            .with_origin(at),
        from: origin.map(|value| -value),
        quarter: 0,
        mirror: 0,
        fill: [0.0; 4],
    };
    let groups = [across.div_ceil(16), down.div_ceil(16), 1];
    let cost = f64::from(across) * f64::from(down);
    let io = Io::new(&stored.plane, &out).limit_writes(paste_area);
    work.groups(Shader::Copy, pasted, io, groups, cost)?;
    work.finish()?;
    Ok(Picture::from_frame(
        picture.imaging(),
        stored.with_plane(out),
    ))
}

pub(crate) fn apply(
    work: &mut Work,
    frame: &Frame,
    step: &Step,
    scale: f32,
    seed: u32,
) -> Result<Frame> {
    match scale == 1.0 {
        true => apply_step(work, frame, step, seed),
        false => apply_step(work, frame, &step.scaled(scale), seed),
    }
}

fn apply_step(work: &mut Work, frame: &Frame, step: &Step, seed: u32) -> Result<Frame> {
    let result = step.operation.apply(work, frame)?;
    if step.is_plain() {
        return Ok(result);
    }
    if result.extent() != frame.extent() || !Arc::ptr_eq(&result.space, &frame.space) {
        return Err(Error::Invalid(
            "reshaping or profile-changing steps cannot be masked, faded or blended".into(),
        ));
    }
    let mask = step
        .mask
        .as_ref()
        .map(|mask| {
            let canvas = work.canvas_or(frame.extent());
            mask.develop(work, canvas, &frame.plane, frame.depth)
        })
        .transpose()?;
    let out = work.plane_like(&frame.plane, frame.depth.working())?;
    let push = BlendPush {
        head: PushHeader::new(&frame.plane, &out)
            .mode(step.blend.code())
            .clamped(frame.depth.clamps()),
        top: result.plane.reference(),
        mask: mask.as_ref().unwrap_or(&result.plane).reference(),
        opacity: step.opacity,
        masked: u32::from(mask.is_some()),
        seed,
        pad: 0,
    };
    let mut io = Io::new(&frame.plane, &out).read(&result.plane);
    if let Some(mask) = &mask {
        io = io.read(mask);
    }
    work.pass(Shader::Blend, push, io, 3.0)?;
    Ok(Frame {
        plane: out,
        has_alpha: frame.has_alpha || result.has_alpha,
        ..result
    })
}

pub(crate) fn store(work: &mut Work, frame: &Frame) -> Result<Frame> {
    let format = frame.depth.storage(work.imaging().formats());
    let out = work.plane_like(&frame.plane, format)?;
    let mode = match frame.mode {
        Mode::Gray => GRAY,
        Mode::Rgb => COPY,
    };
    let push = CopyPush {
        head: PushHeader::new(&frame.plane, &out)
            .mode(mode)
            .clamped(frame.depth.clamps()),
        from: [0, 0],
        quarter: 0,
        mirror: 0,
        fill: [0.0; 4],
    };
    work.pass(Shader::Copy, push, Io::new(&frame.plane, &out), 1.0)?;
    Ok(frame.with_plane(out))
}

crate::work::push_header!(BlendPush);
