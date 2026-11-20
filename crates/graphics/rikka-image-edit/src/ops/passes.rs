use bytemuck::{Pod, Zeroable};
use rikka_gpu::ComputeBuffer;

use crate::error::Result;
use crate::picture::Frame;
use crate::plane::{Plane, saturating_i32};
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

pub(crate) const TO_LINEAR: u32 = 0;
const TO_PREMULTIPLIED: u32 = 1;
pub(crate) const FROM_LINEAR: u32 = 2;
const FROM_PREMULTIPLIED: u32 = 3;

const MIN_SIGMA: f32 = 0.25;

const DIRECT_SIGMA: f32 = 24.0;

const PYRAMID_SIGMA: f32 = 16.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct CopyPush {
    pub(crate) head: PushHeader,
    pub(crate) from: [i32; 2],
    pub(crate) quarter: u32,
    pub(crate) mirror: u32,
    pub(crate) fill: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Domain {
    Linear,
    Encoded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kernel1d {
    Nearest = 0,
    Box = 1,
    Triangle = 2,
    CatmullRom = 3,
    Mitchell = 4,
    Lanczos3 = 5,
    BSpline = 6,
}

impl Kernel1d {
    pub(crate) fn support(self) -> f32 {
        match self {
            Kernel1d::Nearest | Kernel1d::Box => 0.5,
            Kernel1d::Triangle => 1.0,
            Kernel1d::CatmullRom | Kernel1d::Mitchell | Kernel1d::BSpline => 2.0,
            Kernel1d::Lanczos3 => 3.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LinearPush {
    head: PushHeader,
    transfer: u32,
    pad: u32,
    tables: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GaussPush {
    head: PushHeader,
    sigma: f32,
    radius: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ResamplePush {
    head: PushHeader,
    scale: f32,
    support: f32,
    filter: u32,
    start: i32,
    offset: i32,
    limit: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Horizontal = 0,
    Vertical = 1,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResampleAxis {
    pub(crate) direction: Direction,
    pub(crate) scale: f32,
    pub(crate) start: u32,
    pub(crate) offset: u32,
    pub(crate) limit: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AdjustPush {
    head: PushHeader,
    params: u64,
    tables: u64,
    count: u32,
    transfer: u32,
}

pub(crate) fn working_plane(work: &mut Work, frame: &Frame) -> Result<Plane> {
    work.plane(frame.extent(), frame.depth.working())
}

pub(crate) fn premultiplied(work: &mut Work, frame: &Frame, domain: Domain) -> Result<Plane> {
    let out = working_plane(work, frame)?;
    let mode = match domain {
        Domain::Linear => TO_LINEAR,
        Domain::Encoded => TO_PREMULTIPLIED,
    };
    linear_pass(work, frame, &frame.plane, &out, mode, false)?;
    Ok(out)
}

pub(crate) fn straightened(
    work: &mut Work,
    frame: &Frame,
    plane: &Plane,
    domain: Domain,
) -> Result<Plane> {
    let out = work.plane(plane.extent(), frame.depth.working())?;
    let mode = match domain {
        Domain::Linear => FROM_LINEAR,
        Domain::Encoded => FROM_PREMULTIPLIED,
    };
    linear_pass(work, frame, plane, &out, mode, frame.depth.clamps())?;
    Ok(out)
}

pub(crate) fn in_premultiplied(
    work: &mut Work,
    frame: &Frame,
    domain: Domain,
    process: impl FnOnce(&mut Work, Plane) -> Result<Plane>,
) -> Result<Frame> {
    let premultiplied = premultiplied(work, frame, domain)?;
    let processed = process(work, premultiplied)?;
    Ok(frame.with_plane(straightened(work, frame, &processed, domain)?))
}

fn linear_pass(
    work: &mut Work,
    frame: &Frame,
    from: &Plane,
    to: &Plane,
    mode: u32,
    clamp: bool,
) -> Result<()> {
    linear_part(work, frame, [from, to], mode, clamp, to.extent())
}

pub(crate) fn linear_part(
    work: &mut Work,
    frame: &Frame,
    [from, to]: [&Plane; 2],
    mode: u32,
    clamp: bool,
    extent: [u32; 2],
) -> Result<()> {
    let tables = frame.space.tables(work)?;
    let push = LinearPush {
        head: PushHeader::new(from, to).mode(mode).clamped(clamp),
        transfer: frame.space.transfer(),
        pad: 0,
        tables: tables.address(),
    };
    let io = Io::new(from, to).read_buffer(&tables);
    work.pass_over(Shader::Linear, push, io, extent, 1.0)
}

pub(crate) fn gaussian_reach(sigma: f32) -> Option<u32> {
    match sigma {
        sigma if sigma < MIN_SIGMA => Some(0),
        sigma if sigma <= DIRECT_SIGMA => Some((sigma * 3.0).ceil() as u32),
        _ => None,
    }
}

pub(crate) fn gaussian(work: &mut Work, plane: &Plane, sigma: f32) -> Result<Plane> {
    if sigma < MIN_SIGMA {
        return Ok(plane.clone());
    }
    if sigma <= DIRECT_SIGMA {
        return separable_gaussian(work, plane, sigma);
    }
    let levels = (sigma / PYRAMID_SIGMA).log2().ceil().max(1.0) as i32;
    let factor = 2f32.powi(levels);
    let small = [
        (plane.width() as f32 / factor).ceil().max(1.0) as u32,
        (plane.height() as f32 / factor).ceil().max(1.0) as u32,
    ];
    let down = resampled(work, plane, small, Kernel1d::Box)?;
    let box_variance = (factor * factor - 1.0) / 12.0;
    let spline_variance = 1.0 / 3.0;
    let remaining = (sigma * sigma - box_variance) / (factor * factor) - spline_variance;
    let blurred = match remaining > 0.0 {
        true => separable_gaussian(work, &down, remaining.sqrt())?,
        false => down,
    };
    resampled(work, &blurred, plane.extent(), Kernel1d::BSpline)
}

fn separable_gaussian(work: &mut Work, plane: &Plane, sigma: f32) -> Result<Plane> {
    let radius = (sigma * 3.0).ceil() as i32;
    let horizontal = work.plane(plane.extent(), plane.format())?;
    let push = GaussPush {
        head: PushHeader::new(plane, &horizontal).mode(Direction::Horizontal as u32),
        sigma,
        radius,
    };
    work.pass(
        Shader::Gauss,
        push,
        Io::new(plane, &horizontal),
        f64::from(radius * 2 + 1),
    )?;
    let vertical = work.plane(plane.extent(), plane.format())?;
    let push = GaussPush {
        head: PushHeader::new(&horizontal, &vertical).mode(Direction::Vertical as u32),
        sigma,
        radius,
    };
    work.pass(
        Shader::Gauss,
        push,
        Io::new(&horizontal, &vertical),
        f64::from(radius * 2 + 1),
    )?;
    Ok(vertical)
}

pub(crate) fn resampled(
    work: &mut Work,
    plane: &Plane,
    extent: [u32; 2],
    kernel: Kernel1d,
) -> Result<Plane> {
    let wide = [extent[0], plane.height()];
    let horizontal = match extent[0] == plane.width() {
        true => plane.clone(),
        false => {
            let out = work.plane(wide, plane.format())?;
            resample_pass(
                work,
                plane,
                &out,
                Direction::Horizontal,
                extent[0] as f32 / plane.width() as f32,
                kernel,
            )?;
            out
        }
    };
    if extent[1] == plane.height() {
        return Ok(horizontal);
    }
    let out = work.plane(extent, plane.format())?;
    resample_pass(
        work,
        &horizontal,
        &out,
        Direction::Vertical,
        extent[1] as f32 / plane.height() as f32,
        kernel,
    )?;
    Ok(out)
}

fn resample_pass(
    work: &mut Work,
    from: &Plane,
    to: &Plane,
    direction: Direction,
    scale: f32,
    kernel: Kernel1d,
) -> Result<()> {
    let axis = ResampleAxis {
        direction,
        scale,
        start: 0,
        offset: 0,
        limit: from.extent()[direction as usize],
    };
    resample_part(work, [from, to], axis, kernel, to.extent())
}

pub(crate) fn resample_part(
    work: &mut Work,
    [from, to]: [&Plane; 2],
    axis: ResampleAxis,
    kernel: Kernel1d,
    extent: [u32; 2],
) -> Result<()> {
    let support = kernel.support();
    let taps = 2.0 * support / axis.scale.min(1.0) + 1.0;
    let push = ResamplePush {
        head: PushHeader::new(from, to).mode(axis.direction as u32),
        scale: axis.scale,
        support,
        filter: kernel as u32,
        start: saturating_i32(axis.start),
        offset: saturating_i32(axis.offset),
        limit: saturating_i32(axis.limit),
    };
    let io = Io::new(from, to);
    work.pass_over(Shader::Resample, push, io, extent, f64::from(taps))
}

pub(crate) fn adjust(
    work: &mut Work,
    frame: &Frame,
    kind: u32,
    params: &[f32],
    count: u32,
) -> Result<Frame> {
    let buffer = work.constants(params)?;
    adjust_with(work, frame, kind, &buffer, count)
}

pub(crate) fn adjust_with(
    work: &mut Work,
    frame: &Frame,
    kind: u32,
    params: &ComputeBuffer,
    count: u32,
) -> Result<Frame> {
    let out = working_plane(work, frame)?;
    let tables = frame.space.tables(work)?;
    let push = AdjustPush {
        head: PushHeader::new(&frame.plane, &out)
            .mode(kind)
            .clamped(frame.depth.clamps()),
        params: params.address(),
        tables: tables.address(),
        count,
        transfer: frame.space.transfer(),
    };
    work.pass(
        Shader::Adjust,
        push,
        Io::new(&frame.plane, &out)
            .read_buffer(params)
            .read_buffer(&tables),
        4.0,
    )?;
    Ok(frame.with_plane(out))
}

pub(crate) fn scaled_extent(extent: [u32; 2], factor: f32) -> [u32; 2] {
    extent.map(|side| ((side as f32 * factor).round() as u32).max(1))
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ArithPush {
    head: PushHeader,
    other: crate::plane::PlaneRef,
    k: [f32; 4],
    m: [f32; 4],
}

pub(crate) mod arith {
    pub(crate) const RATIO: u32 = 0;
    pub(crate) const MULTIPLY: u32 = 1;
    pub(crate) const SCREEN: u32 = 2;
    pub(crate) const BRIGHT: u32 = 3;
    pub(crate) const MIX: u32 = 4;
    pub(crate) const FOCUS: u32 = 5;
    pub(crate) const OVER: u32 = 6;
}

pub(crate) fn arith(
    work: &mut Work,
    a: &Plane,
    b: &Plane,
    mode: u32,
    k: [f32; 4],
    m: [f32; 4],
) -> Result<Plane> {
    let out = work.plane(a.extent(), a.format())?;
    let push = ArithPush {
        head: PushHeader::new(a, &out).mode(mode),
        other: b.reference(),
        k,
        m,
    };
    work.pass(Shader::Arith, push, Io::new(a, &out).read(b), 2.0)?;
    Ok(out)
}

pub(crate) fn composite_over(
    work: &mut Work,
    frame: &Frame,
    top: &Plane,
    at: [i32; 2],
) -> Result<Plane> {
    let out = work.plane(frame.extent(), frame.depth.working())?;
    let push = ArithPush {
        head: PushHeader::new(&frame.plane, &out)
            .mode(arith::OVER)
            .clamped(frame.depth.clamps()),
        other: top.reference(),
        k: [at[0] as f32, at[1] as f32, 0.0, 0.0],
        m: [0.0; 4],
    };
    work.pass(
        Shader::Arith,
        push,
        Io::new(&frame.plane, &out).read(top),
        2.0,
    )?;
    Ok(out)
}

crate::work::push_header!(
    CopyPush,
    LinearPush,
    GaussPush,
    ResamplePush,
    AdjustPush,
    ArithPush
);
