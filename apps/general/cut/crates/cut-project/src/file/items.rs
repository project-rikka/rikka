use rikka_figure::{Band, Lettering, Look, Pen, Shadow, Shape, TextAlign};
use rikka_types::{Color, Point};
use serde::{Deserialize, Serialize};

use super::is_span;
use crate::camera::Camera;
use crate::footage::{Fade, FadeShape, Mirror, Mix, Treatment};
use crate::graphic::{Art, Graphic};
use crate::placement::Placement;
use crate::region::{Area, Region};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredTransform {
    #[serde(default, skip_serializing_if = "is_zero")]
    depth: f32,
    #[serde(default, skip_serializing_if = "is_origin")]
    anchor: [f32; 2],
    #[serde(default, skip_serializing_if = "is_zero")]
    aspect: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    tilt_x: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    tilt_y: f32,
    #[serde(default, skip_serializing_if = "is_flat")]
    three_d: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredRegion {
    x: f32,
    y: f32,
    zoom: f32,
    rotation: f32,
    opacity: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transform: Option<StoredTransform>,
    area: StoredArea,
    feather: f32,
    inverted: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum StoredArea {
    Whole,
    Shape {
        shape: StoredShape,
        width: f32,
        height: f32,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredTreatment {
    x: f32,
    y: f32,
    zoom: f32,
    rotation: f32,
    opacity: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transform: Option<StoredTransform>,
    radius: f32,
    mirror_horizontal: bool,
    mirror_vertical: bool,
    speed: f32,
    volume: f32,
    pan: f32,
    muted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fade_in: Option<StoredFade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fade_out: Option<StoredFade>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredFade {
    length: i64,
    shape: StoredFadeShape,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredFadeShape {
    Linear,
    EqualPower,
    SCurve,
    Exponential,
}

fn stored_fade(fade: Fade) -> Option<StoredFade> {
    fade.is_on().then_some(StoredFade {
        length: fade.length,
        shape: match fade.shape {
            FadeShape::Linear => StoredFadeShape::Linear,
            FadeShape::EqualPower => StoredFadeShape::EqualPower,
            FadeShape::SCurve => StoredFadeShape::SCurve,
            FadeShape::Exponential => StoredFadeShape::Exponential,
        },
    })
}

fn restored_fade(fade: Option<StoredFade>, clip: u64) -> Result<Fade, String> {
    let Some(fade) = fade else {
        return Ok(Fade::default());
    };
    if !is_span(fade.length) {
        return Err(format!("the sound of clip {clip} fades backwards"));
    }
    Ok(Fade {
        length: fade.length,
        shape: match fade.shape {
            StoredFadeShape::Linear => FadeShape::Linear,
            StoredFadeShape::EqualPower => FadeShape::EqualPower,
            StoredFadeShape::SCurve => FadeShape::SCurve,
            StoredFadeShape::Exponential => FadeShape::Exponential,
        },
    })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredGraphic {
    x: f32,
    y: f32,
    zoom: f32,
    rotation: f32,
    opacity: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transform: Option<StoredTransform>,
    art: StoredArt,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredCamera {
    interest: [f32; 2],
    dolly: f32,
    zoom: f32,
    roll: f32,
    pan: f32,
    tilt: f32,
    angle: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum StoredArt {
    Shape {
        shape: StoredShape,
        width: f32,
        height: f32,
        fill: Option<[f32; 4]>,
        outline: Option<StoredPen>,
    },
    Line {
        reach: [f32; 2],
        pen: StoredPen,
        arrow: bool,
    },
    Text(StoredText),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "form", rename_all = "snake_case", deny_unknown_fields)]
enum StoredShape {
    Rectangle { radius: f32 },
    Ellipse,
    Triangle,
    Polygon { sides: u32 },
    Star { points: u32, depth: f32 },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredPen {
    color: [f32; 4],
    width: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredText {
    text: String,
    font: String,
    size: f32,
    weight: u16,
    italic: bool,
    align: StoredAlign,
    color: [f32; 4],
    letter_spacing: f32,
    line_spacing: f32,
    outlines: Vec<StoredPen>,
    shadow: Option<StoredShadow>,
    band: Option<StoredBand>,
    #[serde(default = "shown", skip_serializing_if = "is_shown")]
    reveal: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredAlign {
    Left,
    Center,
    Right,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredShadow {
    color: [f32; 4],
    offset: [f32; 2],
    blur: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredBand {
    color: [f32; 4],
    padding: f32,
    radius: f32,
}

fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

fn is_origin(value: &[f32; 2]) -> bool {
    *value == [0.0, 0.0]
}

fn is_flat(value: &bool) -> bool {
    !*value
}

fn shown() -> f32 {
    1.0
}

fn is_shown(value: &f32) -> bool {
    *value == 1.0
}

fn stored_transform(placement: &Placement) -> Option<StoredTransform> {
    let rest = Placement::default();
    let moved = placement.depth != rest.depth
        || placement.anchor != rest.anchor
        || placement.aspect != rest.aspect
        || placement.tilt_x != rest.tilt_x
        || placement.tilt_y != rest.tilt_y
        || placement.three_d != rest.three_d;
    moved.then_some(StoredTransform {
        depth: placement.depth,
        anchor: [placement.anchor.x, placement.anchor.y],
        aspect: placement.aspect,
        tilt_x: placement.tilt_x,
        tilt_y: placement.tilt_y,
        three_d: placement.three_d,
    })
}

fn restored_placement(
    [x, y, zoom, rotation, opacity]: [f32; 5],
    transform: Option<&StoredTransform>,
) -> Result<Placement, ()> {
    let mut numbers = vec![x, y, zoom, rotation, opacity];
    let mut placement = Placement {
        x,
        y,
        zoom,
        rotation,
        opacity,
        ..Placement::default()
    };
    if let Some(transform) = transform {
        numbers.extend([
            transform.depth,
            transform.anchor[0],
            transform.anchor[1],
            transform.aspect,
            transform.tilt_x,
            transform.tilt_y,
        ]);
        placement = Placement {
            depth: transform.depth,
            anchor: Point::new(transform.anchor[0], transform.anchor[1]),
            aspect: transform.aspect,
            tilt_x: transform.tilt_x,
            tilt_y: transform.tilt_y,
            three_d: transform.three_d,
            ..placement
        };
    }
    match numbers.iter().all(|number| number.is_finite()) {
        true => Ok(placement.checked()),
        false => Err(()),
    }
}

pub(super) fn stored_color(color: Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

pub(super) fn restored_color(color: [f32; 4]) -> Result<Color, String> {
    match color
        .iter()
        .all(|channel| channel.is_finite() && (0.0..=1.0).contains(channel))
    {
        true => Ok(Color::rgba(color[0], color[1], color[2], color[3])),
        false => Err(format!("{color:?} is not a colour")),
    }
}

fn stored_pen(pen: &Pen) -> StoredPen {
    StoredPen {
        color: stored_color(pen.color),
        width: pen.width,
    }
}

fn restored_pen(pen: &StoredPen) -> Result<Pen, String> {
    match pen.width.is_finite() && pen.width > 0.0 {
        true => Ok(Pen::new(restored_color(pen.color)?, pen.width)),
        false => Err(format!("{} is not a line width", pen.width)),
    }
}

fn stored_shape(shape: Shape) -> StoredShape {
    match shape {
        Shape::Rectangle { radius } => StoredShape::Rectangle { radius },
        Shape::Ellipse => StoredShape::Ellipse,
        Shape::Triangle => StoredShape::Triangle,
        Shape::Polygon { sides } => StoredShape::Polygon { sides },
        Shape::Star { points, depth } => StoredShape::Star { points, depth },
    }
}

fn restored_shape(shape: &StoredShape) -> Shape {
    match *shape {
        StoredShape::Rectangle { radius } => Shape::Rectangle { radius },
        StoredShape::Ellipse => Shape::Ellipse,
        StoredShape::Triangle => Shape::Triangle,
        StoredShape::Polygon { sides } => Shape::Polygon { sides },
        StoredShape::Star { points, depth } => Shape::Star { points, depth },
    }
}

pub(super) fn stored_graphic(graphic: &Graphic) -> StoredGraphic {
    let placement = graphic.placement;
    let art = match &graphic.art {
        Art::Shape {
            shape,
            width,
            height,
            look,
        } => StoredArt::Shape {
            shape: stored_shape(*shape),
            width: *width,
            height: *height,
            fill: look.fill.map(stored_color),
            outline: look.outline.as_ref().map(stored_pen),
        },
        Art::Line { reach, pen, arrow } => StoredArt::Line {
            reach: [reach.x, reach.y],
            pen: stored_pen(pen),
            arrow: *arrow,
        },
        Art::Text { lettering, reveal } => StoredArt::Text(StoredText {
            text: lettering.text.clone(),
            font: lettering.font.clone(),
            size: lettering.size,
            weight: lettering.weight,
            italic: lettering.italic,
            align: match lettering.align {
                TextAlign::Left => StoredAlign::Left,
                TextAlign::Center => StoredAlign::Center,
                TextAlign::Right => StoredAlign::Right,
            },
            color: stored_color(lettering.color),
            letter_spacing: lettering.letter_spacing,
            line_spacing: lettering.line_spacing,
            outlines: lettering.outlines.iter().map(stored_pen).collect(),
            shadow: lettering.shadow.map(|shadow| StoredShadow {
                color: stored_color(shadow.color),
                offset: [shadow.offset.x, shadow.offset.y],
                blur: shadow.blur,
            }),
            band: lettering.band.map(|band| StoredBand {
                color: stored_color(band.color),
                padding: band.padding,
                radius: band.radius,
            }),
            reveal: *reveal,
        }),
    };
    StoredGraphic {
        x: placement.x,
        y: placement.y,
        zoom: placement.zoom,
        rotation: placement.rotation,
        opacity: placement.opacity,
        transform: stored_transform(&placement),
        art,
    }
}

pub(super) fn restored_graphic(graphic: &StoredGraphic, clip: u64) -> Result<Graphic, String> {
    let placement = restored_placement(
        [
            graphic.x,
            graphic.y,
            graphic.zoom,
            graphic.rotation,
            graphic.opacity,
        ],
        graphic.transform.as_ref(),
    )
    .map_err(|()| format!("clip {clip} is placed nowhere"))?;
    let art = match &graphic.art {
        StoredArt::Shape {
            shape,
            width,
            height,
            fill,
            outline,
        } => {
            if !(width.is_finite() && height.is_finite() && *width > 0.0 && *height > 0.0) {
                return Err(format!("the shape of clip {clip} has no size"));
            }
            Art::Shape {
                shape: restored_shape(shape),
                width: *width,
                height: *height,
                look: Look {
                    fill: fill.map(restored_color).transpose()?,
                    outline: outline.as_ref().map(restored_pen).transpose()?,
                },
            }
        }
        StoredArt::Line { reach, pen, arrow } => {
            if !reach.iter().all(|side| side.is_finite()) {
                return Err(format!("the line of clip {clip} reaches nowhere"));
            }
            Art::Line {
                reach: Point::new(reach[0], reach[1]),
                pen: restored_pen(pen)?,
                arrow: *arrow,
            }
        }
        StoredArt::Text(text) => restored_text(text, clip)?,
    };
    Ok(Graphic { placement, art }.checked())
}

fn restored_text(text: &StoredText, clip: u64) -> Result<Art, String> {
    let lettering = Lettering {
        text: text.text.clone(),
        font: text.font.clone(),
        size: text.size,
        weight: text.weight,
        italic: text.italic,
        align: match text.align {
            StoredAlign::Left => TextAlign::Left,
            StoredAlign::Center => TextAlign::Center,
            StoredAlign::Right => TextAlign::Right,
        },
        color: restored_color(text.color)?,
        letter_spacing: text.letter_spacing,
        line_spacing: text.line_spacing,
        outlines: text
            .outlines
            .iter()
            .map(restored_pen)
            .collect::<Result<_, _>>()?,
        shadow: text
            .shadow
            .as_ref()
            .map(|shadow| {
                Ok::<_, String>(Shadow {
                    color: restored_color(shadow.color)?,
                    offset: Point::new(shadow.offset[0], shadow.offset[1]),
                    blur: shadow.blur,
                })
            })
            .transpose()?,
        band: text
            .band
            .as_ref()
            .map(|band| {
                Ok::<_, String>(Band {
                    color: restored_color(band.color)?,
                    padding: band.padding,
                    radius: band.radius,
                })
            })
            .transpose()?,
        ..Lettering::default()
    };
    match lettering.check() {
        Ok(()) | Err(rikka_figure::Error::UnknownFont { .. }) => {}
        Err(trouble) => return Err(format!("the text of clip {clip}: {trouble}")),
    }
    if !text.reveal.is_finite() {
        return Err(format!("the text of clip {clip} reveals nothing"));
    }
    Ok(Art::Text {
        lettering,
        reveal: text.reveal,
    })
}

pub(super) fn stored_treatment(treatment: &Treatment) -> StoredTreatment {
    let placement = treatment.placement;
    StoredTreatment {
        x: placement.x,
        y: placement.y,
        zoom: placement.zoom,
        rotation: placement.rotation,
        opacity: placement.opacity,
        transform: stored_transform(&placement),
        radius: treatment.radius,
        mirror_horizontal: treatment.mirror.horizontal,
        mirror_vertical: treatment.mirror.vertical,
        speed: treatment.speed,
        volume: treatment.mix.volume,
        pan: treatment.mix.pan,
        muted: treatment.mix.muted,
        fade_in: stored_fade(treatment.mix.fade_in),
        fade_out: stored_fade(treatment.mix.fade_out),
    }
}

pub(super) fn restored_treatment(
    treatment: &StoredTreatment,
    clip: u64,
) -> Result<Treatment, String> {
    let nothing = || format!("the footage of clip {clip} is set to nothing");
    let numbers = [
        treatment.radius,
        treatment.speed,
        treatment.volume,
        treatment.pan,
    ];
    if numbers.iter().any(|number| !number.is_finite()) {
        return Err(nothing());
    }
    let placement = restored_placement(
        [
            treatment.x,
            treatment.y,
            treatment.zoom,
            treatment.rotation,
            treatment.opacity,
        ],
        treatment.transform.as_ref(),
    )
    .map_err(|()| nothing())?;
    Ok(Treatment {
        placement,
        radius: treatment.radius,
        mirror: Mirror {
            horizontal: treatment.mirror_horizontal,
            vertical: treatment.mirror_vertical,
        },
        speed: treatment.speed,
        mix: Mix {
            volume: treatment.volume,
            pan: treatment.pan,
            muted: treatment.muted,
            fade_in: restored_fade(treatment.fade_in, clip)?,
            fade_out: restored_fade(treatment.fade_out, clip)?,
        },
    }
    .checked())
}

pub(super) fn stored_region(region: &Region) -> StoredRegion {
    let placement = region.placement;
    StoredRegion {
        x: placement.x,
        y: placement.y,
        zoom: placement.zoom,
        rotation: placement.rotation,
        opacity: placement.opacity,
        transform: stored_transform(&placement),
        area: match region.area {
            Area::Whole => StoredArea::Whole,
            Area::Shape {
                shape,
                width,
                height,
            } => StoredArea::Shape {
                shape: stored_shape(shape),
                width,
                height,
            },
        },
        feather: region.feather,
        inverted: region.inverted,
    }
}

pub(super) fn restored_region(region: &StoredRegion, clip: u64) -> Result<Region, String> {
    let nowhere = || format!("the range of clip {clip} is placed nowhere");
    if !region.feather.is_finite() || region.feather < 0.0 {
        return Err(nowhere());
    }
    let placement = restored_placement(
        [
            region.x,
            region.y,
            region.zoom,
            region.rotation,
            region.opacity,
        ],
        region.transform.as_ref(),
    )
    .map_err(|()| nowhere())?;
    let area = match &region.area {
        StoredArea::Whole => Area::Whole,
        StoredArea::Shape {
            shape,
            width,
            height,
        } => {
            if !(width.is_finite() && height.is_finite() && *width > 0.0 && *height > 0.0) {
                return Err(format!("the range of clip {clip} has no size"));
            }
            Area::Shape {
                shape: restored_shape(shape),
                width: *width,
                height: *height,
            }
        }
    };
    Ok(Region {
        placement,
        area,
        feather: region.feather,
        inverted: region.inverted,
    }
    .checked())
}

pub(super) fn stored_camera(camera: &Camera) -> StoredCamera {
    StoredCamera {
        interest: [camera.interest.x, camera.interest.y],
        dolly: camera.dolly,
        zoom: camera.zoom,
        roll: camera.roll,
        pan: camera.pan,
        tilt: camera.tilt,
        angle: camera.angle,
    }
}

pub(super) fn restored_camera(camera: &StoredCamera, clip: u64) -> Result<Camera, String> {
    let numbers = [
        camera.interest[0],
        camera.interest[1],
        camera.dolly,
        camera.zoom,
        camera.roll,
        camera.pan,
        camera.tilt,
        camera.angle,
    ];
    match numbers.iter().all(|number| number.is_finite()) {
        true => Ok(Camera {
            interest: Point::new(camera.interest[0], camera.interest[1]),
            dolly: camera.dolly,
            zoom: camera.zoom,
            roll: camera.roll,
            pan: camera.pan,
            tilt: camera.tilt,
            angle: camera.angle,
        }
        .checked()),
        false => Err(format!("the camera of clip {clip} looks nowhere")),
    }
}
