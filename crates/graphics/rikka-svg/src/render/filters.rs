use std::sync::LazyLock;

use rikka_types::{Color, Rect, srgb};
use rikka_vector::{Area, BlendMode, Canvas, Transform};

use crate::render::bounds::{placed, scales, within};
use crate::render::lighting::lit;
use crate::render::noise::{Noise, turbulence};
use crate::render::{Host, draw_group, pictures};
use crate::tree::filters::{
    Edge, Filter, ImageContent, Input, Kernel, Kind, Operator, Primitive, Transfer,
};
use crate::tree::{Picture, Units};

const MAX_PIXELS: usize = 1 << 24;
const MAX_KERNEL_WORK: usize = 1 << 30;
const BOX_FROM: f32 = 2.0;

static TO_LINEAR: LazyLock<[u8; 256]> = LazyLock::new(|| {
    std::array::from_fn(|value| (srgb::to_linear(value as f32 / 255.0) * 255.0 + 0.5) as u8)
});

static TO_SRGB: LazyLock<[u8; 256]> = LazyLock::new(|| {
    std::array::from_fn(|value| (srgb::from_linear(value as f32 / 255.0) * 255.0 + 0.5) as u8)
});

#[derive(Clone, Debug)]
pub(crate) struct Image {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) data: Vec<u8>,
    pub(crate) linear: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Window {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
}

impl Image {
    pub(crate) fn blank(width: usize, height: usize, linear: bool) -> Image {
        Image {
            width,
            height,
            data: vec![0; width * height * 4],
            linear,
        }
    }

    fn in_space(mut self, linear: bool) -> Image {
        if self.linear == linear {
            return self;
        }
        let table: &[u8; 256] = match linear {
            true => &TO_LINEAR,
            false => &TO_SRGB,
        };
        for pixel in self.data.as_chunks_mut::<4>().0 {
            let alpha = u32::from(pixel[3]);
            if alpha == 0 {
                continue;
            }
            for channel in &mut pixel[..3] {
                let straight = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255);
                let converted = u32::from(table[straight as usize]);
                *channel = ((converted * alpha + 127) / 255) as u8;
            }
        }
        self.linear = linear;
        self
    }

    fn keep(&mut self, window: Window) {
        for (index, pixel) in self.data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = (index % self.width, index / self.width);
            let inside =
                x >= window.left && x < window.right && y >= window.top && y < window.bottom;
            if !inside {
                *pixel = [0; 4];
            }
        }
    }

    pub(crate) fn pixels(&self) -> &[[u8; 4]] {
        self.data.as_chunks::<4>().0
    }

    fn map(mut self, change: impl Fn([f32; 4]) -> [f32; 4]) -> Image {
        for pixel in self.data.as_chunks_mut::<4>().0 {
            let alpha = f32::from(pixel[3]) / 255.0;
            let straight = match alpha > 0.0 {
                true => [
                    f32::from(pixel[0]) / 255.0 / alpha,
                    f32::from(pixel[1]) / 255.0 / alpha,
                    f32::from(pixel[2]) / 255.0 / alpha,
                    alpha,
                ],
                false => [0.0; 4],
            };
            let [red, green, blue, alpha] = change(straight).map(|value| value.clamp(0.0, 1.0));
            *pixel = stored([red * alpha, green * alpha, blue * alpha, alpha]);
        }
        self
    }

    fn combine(mut self, other: &Image, mix: impl Fn([f32; 4], [f32; 4]) -> [f32; 4]) -> Image {
        for (pixel, below) in self
            .data
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(other.pixels())
        {
            *pixel = stored(mix(loaded(*pixel), loaded(*below)));
        }
        self
    }
}

fn loaded(pixel: [u8; 4]) -> [f32; 4] {
    pixel.map(|channel| f32::from(channel) / 255.0)
}

fn stored(pixel: [f32; 4]) -> [u8; 4] {
    let alpha = pixel[3].clamp(0.0, 1.0);
    let byte = |value: f32| (value.clamp(0.0, alpha) * 255.0 + 0.5) as u8;
    [byte(pixel[0]), byte(pixel[1]), byte(pixel[2]), byte(alpha)]
}

pub(crate) fn apply(
    filter: &Filter,
    object: Option<Rect>,
    transform: &Transform,
    host: Host,
    canvas: &mut Canvas,
) {
    let (area, pixels) = canvas.layer_mut();
    let region = within(filter.rect, filter.units, object)
        .map(|rect| placed(rect, transform))
        .and_then(|device| inside(device, area));
    let Some(region) = region else {
        pixels.fill(0);
        return;
    };
    let (width, height) = (region.right - region.left, region.bottom - region.top);
    if width * height > MAX_PIXELS {
        return;
    }
    let stride = area.width as usize * 4;
    let mut source = Image::blank(width, height, false);
    for row in 0..height {
        let from = (region.top + row) * stride + region.left * 4;
        source.data[row * width * 4..(row + 1) * width * 4]
            .copy_from_slice(&pixels[from..from + width * 4]);
    }
    let frame = Frame {
        primitive_units: filter.primitive_units,
        object,
        transform: *transform,
        host,
        origin: (
            area.left as f32 + region.left as f32,
            area.top as f32 + region.top as f32,
        ),
        whole: Window {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        },
    };
    let mut named: Vec<(&str, Image, Window)> = Vec::new();
    let mut previous: Option<(Image, Window)> = None;
    for primitive in &filter.primitives {
        let window = frame.window(primitive);
        let fetch = |input: &Input| -> (Image, Window) {
            let (image, window) = match input {
                Input::Source => (source.clone(), frame.whole),
                Input::SourceAlpha => {
                    let mut alpha = source.clone();
                    for pixel in alpha.data.as_chunks_mut::<4>().0 {
                        *pixel = [0, 0, 0, pixel[3]];
                    }
                    (alpha, frame.whole)
                }
                Input::Previous => previous
                    .clone()
                    .unwrap_or_else(|| (source.clone(), frame.whole)),
                Input::Named(name) => named
                    .iter()
                    .rev()
                    .find(|(known, _, _)| known == name)
                    .map(|(_, image, window)| (image.clone(), *window))
                    .or_else(|| previous.clone())
                    .unwrap_or_else(|| (source.clone(), frame.whole)),
                Input::Nothing => (Image::blank(width, height, primitive.linear), frame.whole),
            };
            (image.in_space(primitive.linear), window)
        };
        let mut result = frame.run(primitive, window, fetch);
        result.keep(window);
        if !primitive.result.is_empty() {
            let name = primitive.result.as_str();
            match named.iter_mut().find(|(known, _, _)| *known == name) {
                Some(entry) => *entry = (name, result.clone(), window),
                None => named.push((name, result.clone(), window)),
            }
        }
        previous = Some((result, window));
    }
    let result = previous.map_or_else(
        || Image::blank(width, height, false),
        |(image, _)| image.in_space(false),
    );
    pixels.fill(0);
    for row in 0..height {
        let to = (region.top + row) * stride + region.left * 4;
        pixels[to..to + width * 4]
            .copy_from_slice(&result.data[row * width * 4..(row + 1) * width * 4]);
    }
}

fn inside(device: Rect, area: Area) -> Option<Window> {
    let left = (device.min_x().floor() as i64 - i64::from(area.left)).max(0);
    let top = (device.min_y().floor() as i64 - i64::from(area.top)).max(0);
    let right = (device.max_x().ceil() as i64 - i64::from(area.left)).min(i64::from(area.width));
    let bottom = (device.max_y().ceil() as i64 - i64::from(area.top)).min(i64::from(area.height));
    (right > left && bottom > top).then_some(Window {
        left: left as usize,
        top: top as usize,
        right: right as usize,
        bottom: bottom as usize,
    })
}

struct Frame {
    primitive_units: Units,
    object: Option<Rect>,
    transform: Transform,
    host: Host,
    origin: (f32, f32),
    whole: Window,
}

impl Frame {
    fn window(&self, primitive: &Primitive) -> Window {
        let [x, y, width, height] = primitive.region;
        if x.is_none() && y.is_none() && width.is_none() && height.is_none() {
            return self.whole;
        }
        let Some(user) = self.user_region(primitive) else {
            return self.whole;
        };
        let device = placed(user, &self.transform);
        let clamp = |value: f32, limit: usize| (value.round().max(0.0) as usize).min(limit);
        Window {
            left: clamp(device.min_x() - self.origin.0, self.whole.right),
            top: clamp(device.min_y() - self.origin.1, self.whole.bottom),
            right: clamp(device.max_x() - self.origin.0, self.whole.right),
            bottom: clamp(device.max_y() - self.origin.1, self.whole.bottom),
        }
    }

    fn user_region(&self, primitive: &Primitive) -> Option<Rect> {
        let [x, y, width, height] = primitive.region;
        let object = self.object.unwrap_or(Rect::ZERO);
        let boxed = self.primitive_units == Units::BoundingBox;
        let along = |value: Option<f32>, start: f32, size: f32, offset: bool| {
            value.map(|value| match (boxed, offset) {
                (true, true) => start + value * size,
                (true, false) => value * size,
                (false, _) => value,
            })
        };
        let inverse = self.transform.inverse();
        let whole_user = inverse.map(|inverse| {
            let device = Rect::new(
                self.origin.0,
                self.origin.1,
                self.whole.right as f32,
                self.whole.bottom as f32,
            );
            placed(device, &inverse)
        });
        let whole_user = whole_user?;
        Some(Rect::new(
            along(x, object.min_x(), object.width(), true).unwrap_or(whole_user.min_x()),
            along(y, object.min_y(), object.height(), true).unwrap_or(whole_user.min_y()),
            along(width, 0.0, object.width(), false).unwrap_or(whole_user.width()),
            along(height, 0.0, object.height(), false).unwrap_or(whole_user.height()),
        ))
    }

    fn local(&self) -> Transform {
        self.transform
            .then(Transform::translate(-self.origin.0, -self.origin.1))
    }

    fn image(&self, primitive: &Primitive, content: &ImageContent) -> Image {
        let (width, height) = (self.whole.right, self.whole.bottom);
        let mut canvas = Canvas::new();
        canvas.begin(width as u32, height as u32);
        let local = self.local();
        match content {
            ImageContent::Element(group) => draw_group(group, &local, self.host, &mut canvas),
            ImageContent::Picture {
                content,
                aspect,
                smooth,
            } => {
                let Some(frame) = self.user_region(primitive) else {
                    return Image::blank(width, height, false);
                };
                let picture = Picture {
                    id: String::new(),
                    transform: Transform::IDENTITY,
                    frame,
                    aspect: *aspect,
                    smooth: *smooth,
                    content: content.clone(),
                };
                pictures::draw(&picture, &local, self.host, &mut canvas);
            }
        }
        Image {
            width,
            height,
            data: canvas.pixels().to_vec(),
            linear: false,
        }
    }

    fn lengths(&self, pair: (f32, f32)) -> (f32, f32) {
        let scale = scales(&self.transform);
        let (x, y) = match (self.primitive_units, self.object) {
            (Units::BoundingBox, Some(object)) => {
                (pair.0 * object.width(), pair.1 * object.height())
            }
            _ => pair,
        };
        (x * scale.x, y * scale.y)
    }

    fn run(
        &self,
        primitive: &Primitive,
        window: Window,
        fetch: impl Fn(&Input) -> (Image, Window),
    ) -> Image {
        let (width, height) = (self.whole.right, self.whole.bottom);
        let linear = primitive.linear;
        match &primitive.kind {
            Kind::Blur { input, deviation } => {
                let (x, y) = self.lengths(*deviation);
                match x < 0.0 || y < 0.0 || (x == 0.0 && y == 0.0) {
                    true if x == 0.0 && y == 0.0 => Image::blank(width, height, linear),
                    true => fetch(input).0,
                    false => blurred(fetch(input).0, x, y),
                }
            }
            Kind::Offset { input, dx, dy } => {
                let (dx, dy) = self.lengths((*dx, *dy));
                shifted(&fetch(input).0, dx.round() as i64, dy.round() as i64)
            }
            Kind::Flood { color } => flooded(width, height, window, *color, linear),
            Kind::Merge { inputs } => inputs
                .iter()
                .fold(Image::blank(width, height, linear), |below, input| {
                    fetch(input).0.combine(&below, over)
                }),
            Kind::Composite {
                input,
                second,
                operator,
            } => {
                let operator = *operator;
                fetch(input)
                    .0
                    .combine(&fetch(second).0, move |source, below| {
                        composite(operator, source, below)
                    })
            }
            Kind::Blend {
                input,
                second,
                mode,
            } => {
                let mode = *mode;
                fetch(input)
                    .0
                    .combine(&fetch(second).0, move |source, below| match mode {
                        BlendMode::SourceOver => over(source, below),
                        mode => mode.composite(source, below),
                    })
            }
            Kind::ColorMatrix { input, matrix } => {
                let m = *matrix;
                fetch(input).0.map(move |[r, g, b, a]| {
                    std::array::from_fn(|row| {
                        let at = row * 5;
                        m[at] * r + m[at + 1] * g + m[at + 2] * b + m[at + 3] * a + m[at + 4]
                    })
                })
            }
            Kind::Transfer { input, functions } => fetch(input).0.map(|pixel| {
                std::array::from_fn(|channel| transferred(&functions[channel], pixel[channel]))
            }),
            Kind::DropShadow {
                input,
                dx,
                dy,
                deviation,
                color,
            } => {
                let source = fetch(input).0;
                let (x, y) = self.lengths(*deviation);
                let (dx, dy) = self.lengths((*dx, *dy));
                let mut shadow = source.clone();
                let tint = stored(premultiplied(*color, linear));
                for pixel in shadow.data.as_chunks_mut::<4>().0 {
                    let alpha = u32::from(pixel[3]);
                    *pixel = tint.map(|channel| ((u32::from(channel) * alpha + 127) / 255) as u8);
                }
                let shadow = match x > 0.0 || y > 0.0 {
                    true => blurred(shadow, x.max(0.0), y.max(0.0)),
                    false => shadow,
                };
                let shadow = shifted(&shadow, dx.round() as i64, dy.round() as i64);
                source.combine(&shadow, over)
            }
            Kind::Morphology {
                input,
                radius,
                dilate,
            } => {
                let (x, y) = self.lengths(*radius);
                let source = fetch(input).0;
                match x <= 0.0 && y <= 0.0 {
                    true => source,
                    false => morphed(&source, x.round() as usize, y.round() as usize, *dilate),
                }
            }
            Kind::Tile { input } => {
                let (source, from) = fetch(input);
                tiled(&source, from)
            }
            Kind::Image { content } => match content {
                Some(content) => self.image(primitive, content),
                None => Image::blank(width, height, false),
            },
            Kind::Turbulence {
                frequency,
                octaves,
                seed,
                stitch,
                fractal,
            } => {
                let local = self.local();
                let Some(inverse) = local.inverse() else {
                    return Image::blank(width, height, linear);
                };
                let tile = stitch.then(|| self.user_region(primitive)).flatten();
                let noise = Noise {
                    frequency: *frequency,
                    octaves: *octaves,
                    seed: *seed,
                    tile,
                    fractal: *fractal,
                };
                turbulence(width, height, &inverse, noise, linear)
            }
            Kind::Convolve { input, kernel } => {
                let source = fetch(input).0;
                match width * height * kernel.values.len() > MAX_KERNEL_WORK {
                    true => Image::blank(width, height, linear),
                    false => convolved(&source, kernel),
                }
            }
            Kind::Displace {
                input,
                map,
                scale,
                channels,
            } => {
                let (x, y) = self.lengths((*scale, *scale));
                displaced(&fetch(input).0, &fetch(map).0, (x, y), *channels)
            }
            Kind::Lighting { input, lighting } => {
                let local = self.local();
                lit(&fetch(input).0, lighting, &local, linear)
            }
            Kind::Passthrough { input } => fetch(input).0,
        }
    }
}

fn convolved(image: &Image, kernel: &Kernel) -> Image {
    let (width, height) = (image.width as i64, image.height as i64);
    let source = image.pixels();
    let straight: Vec<[f32; 4]> = source
        .iter()
        .map(|pixel| {
            let mut channels = loaded(*pixel);
            let alpha = channels[3];
            if kernel.preserve_alpha && alpha > 0.0 {
                for channel in &mut channels[..3] {
                    *channel /= alpha;
                }
            }
            channels
        })
        .collect();
    let mut out = Image::blank(image.width, image.height, image.linear);
    for (index, pixel) in out.data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = ((index as i64) % width, (index as i64) / width);
        let mut sums = [0.0f32; 4];
        for row in 0..kernel.rows {
            for column in 0..kernel.columns {
                let mut from_x = x - kernel.target.0 as i64 + column as i64;
                let mut from_y = y - kernel.target.1 as i64 + row as i64;
                match kernel.edge {
                    Edge::Transparent => {
                        if from_x < 0 || from_x >= width || from_y < 0 || from_y >= height {
                            continue;
                        }
                    }
                    Edge::Duplicate => {
                        from_x = from_x.clamp(0, width - 1);
                        from_y = from_y.clamp(0, height - 1);
                    }
                    Edge::Wrap => {
                        from_x = from_x.rem_euclid(width);
                        from_y = from_y.rem_euclid(height);
                    }
                }
                let weight = kernel.values
                    [(kernel.rows - 1 - row) * kernel.columns + (kernel.columns - 1 - column)];
                let texel = straight[(from_y * width + from_x) as usize];
                for (sum, value) in sums.iter_mut().zip(texel) {
                    *sum += value * weight;
                }
            }
        }
        let alpha = match kernel.preserve_alpha {
            true => straight[index][3],
            false => (sums[3] / kernel.divisor + kernel.bias).clamp(0.0, 1.0),
        };
        let channel = |sum: f32| {
            let value = sum / kernel.divisor + kernel.bias * alpha;
            match kernel.preserve_alpha {
                true => value.clamp(0.0, 1.0) * alpha,
                false => value.clamp(0.0, alpha),
            }
        };
        *pixel = stored([channel(sums[0]), channel(sums[1]), channel(sums[2]), alpha]);
    }
    out
}

fn displaced(image: &Image, map: &Image, scale: (f32, f32), channels: (usize, usize)) -> Image {
    let (width, height) = (image.width as i64, image.height as i64);
    let mut out = Image::blank(image.width, image.height, image.linear);
    let source = image.pixels();
    for (index, (pixel, guide)) in out
        .data
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(map.pixels())
        .enumerate()
    {
        let alpha = f32::from(guide[3]);
        let straight = |channel: usize| match (channel, alpha > 0.0) {
            (3, _) => alpha / 255.0,
            (_, true) => (f32::from(guide[channel]) / alpha).min(1.0),
            (_, false) => 0.0,
        };
        let (x, y) = ((index as i64) % width, (index as i64) / width);
        let from_x = (x as f32 + scale.0 * (straight(channels.0) - 0.5)).round() as i64;
        let from_y = (y as f32 + scale.1 * (straight(channels.1) - 0.5)).round() as i64;
        if (0..width).contains(&from_x) && (0..height).contains(&from_y) {
            *pixel = source[(from_y * width + from_x) as usize];
        }
    }
    out
}

pub(crate) fn premultiplied(color: Color, linear: bool) -> [f32; 4] {
    let channel = |value: f32| match linear {
        true => srgb::to_linear(value.clamp(0.0, 1.0)),
        false => value.clamp(0.0, 1.0),
    };
    let alpha = color.a.clamp(0.0, 1.0);
    [
        channel(color.r) * alpha,
        channel(color.g) * alpha,
        channel(color.b) * alpha,
        alpha,
    ]
}

fn over(source: [f32; 4], below: [f32; 4]) -> [f32; 4] {
    let keep = 1.0 - source[3];
    std::array::from_fn(|channel| source[channel] + below[channel] * keep)
}

fn composite(operator: Operator, source: [f32; 4], below: [f32; 4]) -> [f32; 4] {
    let (sa, ba) = (source[3], below[3]);
    let weighted = |fa: f32, fb: f32| -> [f32; 4] {
        std::array::from_fn(|channel| source[channel] * fa + below[channel] * fb)
    };
    match operator {
        Operator::Over => weighted(1.0, 1.0 - sa),
        Operator::In => weighted(ba, 0.0),
        Operator::Out => weighted(1.0 - ba, 0.0),
        Operator::Atop => weighted(ba, 1.0 - sa),
        Operator::Xor => weighted(1.0 - ba, 1.0 - sa),
        Operator::Arithmetic([k1, k2, k3, k4]) => {
            let mut mixed: [f32; 4] = std::array::from_fn(|channel| {
                (k1 * source[channel] * below[channel]
                    + k2 * source[channel]
                    + k3 * below[channel]
                    + k4)
                    .clamp(0.0, 1.0)
            });
            for channel in 0..3 {
                mixed[channel] = mixed[channel].min(mixed[3]);
            }
            mixed
        }
    }
}

fn transferred(function: &Transfer, value: f32) -> f32 {
    match function {
        Transfer::Identity => value,
        Transfer::Table(table) => {
            let last = table.len() - 1;
            let position = value * last as f32;
            let at = (position.floor() as usize).min(last);
            let next = (at + 1).min(last);
            table[at] + (table[next] - table[at]) * (position - at as f32)
        }
        Transfer::Discrete(table) => {
            let at = ((value * table.len() as f32).floor() as usize).min(table.len() - 1);
            table[at]
        }
        Transfer::Linear { slope, intercept } => slope * value + intercept,
        Transfer::Gamma {
            amplitude,
            exponent,
            offset,
        } => amplitude * value.powf(*exponent) + offset,
    }
}

fn flooded(width: usize, height: usize, window: Window, color: Color, linear: bool) -> Image {
    let mut image = Image::blank(width, height, linear);
    let ink = stored(premultiplied(color, linear));
    for y in window.top..window.bottom {
        let row = &mut image.data[(y * width + window.left) * 4..(y * width + window.right) * 4];
        for pixel in row.as_chunks_mut::<4>().0 {
            *pixel = ink;
        }
    }
    image
}

fn shifted(image: &Image, dx: i64, dy: i64) -> Image {
    let mut moved = Image::blank(image.width, image.height, image.linear);
    let (width, height) = (image.width as i64, image.height as i64);
    let (dx, dy) = (dx.clamp(-width, width), dy.clamp(-height, height));
    for y in 0..height {
        let from_y = y - dy;
        if from_y < 0 || from_y >= height {
            continue;
        }
        let (start, end) = (dx.max(0), (width + dx).min(width));
        if start >= end {
            continue;
        }
        let count = (end - start) as usize * 4;
        let to = (y * width + start) as usize * 4;
        let from = (from_y * width + start - dx) as usize * 4;
        moved.data[to..to + count].copy_from_slice(&image.data[from..from + count]);
    }
    moved
}

fn tiled(image: &Image, from: Window) -> Image {
    let (tile_width, tile_height) = (from.right - from.left, from.bottom - from.top);
    if tile_width == 0 || tile_height == 0 {
        return Image::blank(image.width, image.height, image.linear);
    }
    let mut out = Image::blank(image.width, image.height, image.linear);
    let source = image.pixels();
    for (index, pixel) in out.data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (index % image.width, index / image.width);
        let wrap = |value: usize, start: usize, size: usize| {
            start + (value as i64 - start as i64).rem_euclid(size as i64) as usize
        };
        let at = wrap(y, from.top, tile_height) * image.width + wrap(x, from.left, tile_width);
        *pixel = source[at];
    }
    out
}

fn morphed(image: &Image, rx: usize, ry: usize, dilate: bool) -> Image {
    let pick = |one: u8, other: u8| match dilate {
        true => one.max(other),
        false => one.min(other),
    };
    let seed = if dilate { 0u8 } else { 255u8 };
    let (width, height) = (image.width, image.height);
    let (rx, ry) = (rx.min(width), ry.min(height));
    let source = image.pixels();
    let mut across = vec![[0u8; 4]; width * height];
    for y in 0..height {
        for x in 0..width {
            let mut best = [seed; 4];
            for near in x.saturating_sub(rx)..(x + rx + 1).min(width) {
                let pixel = source[y * width + near];
                best = std::array::from_fn(|channel| pick(best[channel], pixel[channel]));
            }
            if !dilate && (x < rx || x + rx >= width) {
                best = [0; 4];
            }
            across[y * width + x] = best;
        }
    }
    let mut out = Image::blank(width, height, image.linear);
    for (index, pixel) in out.data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (index % width, index / width);
        let mut best = [seed; 4];
        for near in y.saturating_sub(ry)..(y + ry + 1).min(height) {
            let value = across[near * width + x];
            best = std::array::from_fn(|channel| pick(best[channel], value[channel]));
        }
        if !dilate && (y < ry || y + ry >= height) {
            best = [0; 4];
        }
        *pixel = best;
    }
    out
}

fn blurred(mut image: Image, x: f32, y: f32) -> Image {
    let (width, height) = (image.width, image.height);
    let mut scratch = vec![0u8; image.data.len()];
    if x > 0.0 {
        blur_lines(
            &mut image.data,
            &mut scratch,
            width,
            height,
            4,
            width * 4,
            x,
        );
    }
    if y > 0.0 {
        blur_lines(
            &mut image.data,
            &mut scratch,
            height,
            width,
            width * 4,
            4,
            y,
        );
    }
    image
}

fn blur_lines(
    data: &mut [u8],
    scratch: &mut [u8],
    length: usize,
    lines: usize,
    step: usize,
    line_step: usize,
    deviation: f32,
) {
    let deviation = deviation.min(length as f32);
    if deviation < BOX_FROM {
        let radius = (deviation * 3.0).ceil() as usize;
        let weights: Vec<f32> = (0..=radius * 2)
            .map(|at| {
                let distance = at as f32 - radius as f32;
                (-distance * distance / (2.0 * deviation * deviation)).exp()
            })
            .collect();
        let total: f32 = weights.iter().sum();
        for line in 0..lines {
            for at in 0..length {
                let mut sums = [0.0f32; 4];
                for (tap, weight) in weights.iter().enumerate() {
                    let Some(from) = (at + tap).checked_sub(radius).filter(|from| *from < length)
                    else {
                        continue;
                    };
                    let start = line * line_step + from * step;
                    for (sum, channel) in sums.iter_mut().zip(&data[start..start + 4]) {
                        *sum += f32::from(*channel) * weight;
                    }
                }
                let start = line * line_step + at * step;
                for (target, sum) in scratch[start..start + 4].iter_mut().zip(sums) {
                    *target = (sum / total + 0.5) as u8;
                }
            }
        }
        data.copy_from_slice(scratch);
        return;
    }
    let size = (deviation * 3.0 * (2.0 * std::f32::consts::PI).sqrt() / 4.0 + 0.5).floor() as usize;
    let size = size.max(1);
    let passes = match size % 2 {
        1 => [(size, size / 2), (size, size / 2), (size, size / 2)],
        _ => [(size, size / 2), (size, size / 2 - 1), (size + 1, size / 2)],
    };
    for (size, before) in passes {
        for line in 0..lines {
            let mut sums = [0u32; 4];
            let texel = |at: usize| -> [u8; 4] {
                let start = line * line_step + at * step;
                [
                    data[start],
                    data[start + 1],
                    data[start + 2],
                    data[start + 3],
                ]
            };
            for at in 0..(size - before).min(length) {
                for (sum, channel) in sums.iter_mut().zip(texel(at)) {
                    *sum += u32::from(channel);
                }
            }
            for at in 0..length {
                let start = line * line_step + at * step;
                for (target, sum) in scratch[start..start + 4].iter_mut().zip(sums) {
                    *target = ((sum + size as u32 / 2) / size as u32) as u8;
                }
                let entering = at + size - before;
                if entering < length {
                    for (sum, channel) in sums.iter_mut().zip(texel(entering)) {
                        *sum += u32::from(channel);
                    }
                }
                if let Some(leaving) = at.checked_sub(before) {
                    for (sum, channel) in sums.iter_mut().zip(texel(leaving)) {
                        *sum -= u32::from(channel);
                    }
                }
            }
        }
        data.copy_from_slice(scratch);
    }
}
