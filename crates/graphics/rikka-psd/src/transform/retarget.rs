use crate::descriptor::{Descriptor, Value};
use crate::document::Document;
use crate::geometry::Rect;
use crate::info::{Info, MetadataValue};
use crate::layers::Layer;
use crate::path::{PathData, Point};
use crate::resources::{
    GridAndGuides, Guide, GuideDirection, Resolution, ResourceData, id as resource_id,
};
use crate::style::Effects;
use crate::transform::Affine;
use crate::transform::records::each_mut;

const ALIGNED: f64 = 1e-9;
const SMART_OBJECT_TRANSFORMS: [&str; 2] = ["Trnf", "nonAffineTransform"];
const CORNERS: [&str; 4] = [
    "rectangleCornerA",
    "rectangleCornerB",
    "rectangleCornerC",
    "rectangleCornerD",
];
const RADII: [(&str, (f64, f64)); 4] = [
    ("topLeft", (-1.0, -1.0)),
    ("topRight", (1.0, -1.0)),
    ("bottomRight", (1.0, 1.0)),
    ("bottomLeft", (-1.0, 1.0)),
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct Retarget {
    map: Affine,
    from: (f64, f64),
    to: (f64, f64),
    styles: f64,
}

impl Retarget {
    pub(crate) fn new(map: Affine, from: (u32, u32), to: (u32, u32)) -> Self {
        Self {
            map,
            from: (f64::from(from.0), f64::from(from.1)),
            to: (f64::from(to.0), f64::from(to.1)),
            styles: 1.0,
        }
    }

    pub(crate) fn scaling_styles(mut self, scale: bool) -> Self {
        self.styles = if scale { self.map.mean_scale() } else { 1.0 };
        self
    }

    pub(crate) fn apply(self, document: &mut Document) {
        each_mut(&mut document.layers, &mut |layer| self.layer(layer));
        for info in &mut document.info {
            self.info(info);
        }
        for resource in &mut document.resources.items {
            match &mut resource.data {
                ResourceData::Path(path) => self.path(path),
                ResourceData::GridAndGuides(grid) => self.guides(grid),
                ResourceData::Resolution(resolution) if self.map.swaps_axes() => {
                    *resolution = Resolution {
                        horizontal: resolution.vertical,
                        horizontal_unit: resolution.vertical_unit,
                        width_unit: resolution.height_unit,
                        vertical: resolution.horizontal,
                        vertical_unit: resolution.horizontal_unit,
                        height_unit: resolution.width_unit,
                    };
                }
                ResourceData::PixelAspectRatio { ratio, .. }
                    if self.map.swaps_axes() && ratio.is_normal() =>
                {
                    *ratio = 1.0 / *ratio;
                }
                ResourceData::Descriptor { descriptor, .. } => match resource.id {
                    resource_id::SLICES => self.slices(descriptor),
                    resource_id::ORIGIN_PATH => self.origination(descriptor),
                    _ => {}
                },
                _ => {}
            }
        }
        for stale in [resource_id::THUMBNAIL, resource_id::THUMBNAIL_BGR] {
            document.resources.remove(stale);
        }
    }

    fn point(self, x: f64, y: f64) -> (f64, f64) {
        self.map.apply(x, y)
    }

    fn fraction(self, point: &mut Point) {
        let (x, y) = self.point(point.x * self.from.0, point.y * self.from.1);
        point.x = x / self.to.0;
        point.y = y / self.to.1;
    }

    fn span(self, [left, top, right, bottom]: [f64; 4]) -> [f64; 4] {
        let corners = [(left, top), (right, top), (right, bottom), (left, bottom)]
            .map(|(x, y)| self.point(x, y));
        let xs = corners.map(|corner| corner.0);
        let ys = corners.map(|corner| corner.1);
        let low = |values: [f64; 4]| values.into_iter().fold(f64::INFINITY, f64::min);
        let high = |values: [f64; 4]| values.into_iter().fold(f64::NEG_INFINITY, f64::max);
        [low(xs), low(ys), high(xs), high(ys)]
    }

    fn rect(self, rect: &mut Rect) {
        if rect.is_empty() {
            return;
        }
        let [left, top, right, bottom] = self
            .span([rect.left, rect.top, rect.right, rect.bottom].map(f64::from))
            .map(|value| {
                value
                    .round()
                    .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
            });
        *rect = Rect::new(left, top, right, bottom);
    }

    fn angle(self, degrees: f64) -> f64 {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let (x, y) = self.map.apply_vector(cos, -sin);
        let mapped = (-y).atan2(x).to_degrees();
        if mapped <= -180.0 {
            mapped + 360.0
        } else {
            mapped
        }
    }

    fn layer(self, layer: &mut Layer) {
        if let Some(mask) = &mut layer.mask {
            let scale = self.map.mean_scale();
            for feather in [
                &mut mask.parameters.user_feather,
                &mut mask.parameters.vector_feather,
            ]
            .into_iter()
            .flatten()
            {
                *feather *= scale;
            }
        }
        for info in &mut layer.info {
            self.info(info);
        }
    }

    fn info(self, info: &mut Info) {
        match info {
            Info::VectorMask(mask) => self.path(&mut mask.path),
            Info::TypeTool(tool) => {
                tool.transform = Affine::from_array(tool.transform).then(self.map).to_array();
            }
            Info::PlacedLayer(placed) => self.corners(&mut placed.transform),
            Info::ReferencePoint { x, y } => (*x, *y) = self.point(*x, *y),
            Info::Annotations(annotations) => {
                for annotation in &mut annotations.items {
                    self.rect(&mut annotation.icon);
                    self.rect(&mut annotation.popup);
                }
            }
            Info::Metadata(items) => {
                for item in items {
                    if let MetadataValue::Descriptor(descriptor) = &mut item.value {
                        match &item.key.0 {
                            b"cmls" => self.layer_comps(descriptor),
                            b"mlst" => self.frames(descriptor),
                            _ => {}
                        }
                    }
                }
            }
            Info::Descriptor(block) => {
                let descriptor = &mut block.descriptor;
                match &block.key.0 {
                    b"SoLd" | b"SoLE" => self.smart_object(descriptor),
                    b"vogk" => self.origination(descriptor),
                    b"artb" | b"abdd" => {
                        if let Some(rect) = descriptor.descriptor_mut("artboardRect") {
                            self.rect_descriptor(rect);
                        }
                    }
                    b"GdFl" | b"vscg" => self.gradient(descriptor),
                    b"PtFl" => scale_number(descriptor, "Scl ", self.styles),
                    b"vstk" => {
                        scale_number(descriptor, "strokeStyleLineWidth", self.styles);
                        if let Some(content) = descriptor.descriptor_mut("strokeStyleContent") {
                            self.gradient(content);
                        }
                    }
                    b"lfx2" | b"lmfx" | b"lfxs" if self.styles != 1.0 => {
                        let mut effects = Effects::from_descriptor(descriptor);
                        effects.scale_sizes(self.styles);
                        *descriptor = effects.to_source_layout();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn path(self, path: &mut PathData) {
        for subpath in path.subpaths_mut() {
            for knot in &mut subpath.knots {
                self.fraction(&mut knot.preceding);
                self.fraction(&mut knot.anchor);
                self.fraction(&mut knot.leaving);
            }
        }
    }

    fn guides(self, grid: &mut GridAndGuides) {
        grid.guides = grid
            .guides
            .iter()
            .filter_map(|guide| {
                let (point, along) = match guide.direction {
                    GuideDirection::Vertical => ((guide.position, 0.0), (0.0, 1.0)),
                    GuideDirection::Horizontal => ((0.0, guide.position), (1.0, 0.0)),
                };
                let (x, y) = self.point(point.0, point.1);
                let (dx, dy) = self.map.apply_vector(along.0, along.1);
                if dx.abs() < ALIGNED {
                    Some(Guide {
                        position: x,
                        direction: GuideDirection::Vertical,
                    })
                } else if dy.abs() < ALIGNED {
                    Some(Guide {
                        position: y,
                        direction: GuideDirection::Horizontal,
                    })
                } else {
                    None
                }
            })
            .collect();
    }

    fn corners(self, values: &mut [f64]) {
        for [x, y] in values.as_chunks_mut::<2>().0 {
            (*x, *y) = self.point(*x, *y);
        }
    }

    fn smart_object(self, descriptor: &mut Descriptor) {
        for key in SMART_OBJECT_TRANSFORMS {
            let Some(Value::List(values)) = descriptor.get_mut(key) else {
                continue;
            };
            let mut numbers: Vec<f64> = values.iter().filter_map(Value::number).collect();
            if numbers.len() != values.len() {
                continue;
            }
            self.corners(&mut numbers);
            for (value, number) in values.iter_mut().zip(numbers) {
                set_number(value, number);
            }
        }
    }

    fn origination(self, descriptor: &mut Descriptor) {
        let Some(Value::List(items)) = descriptor.get_mut("keyDescriptorList") else {
            return;
        };
        for item in items {
            let (Value::Descriptor(item) | Value::GlobalObject(item)) = item else {
                continue;
            };
            if let Some(corners) = item.descriptor_mut("keyOriginBoxCorners") {
                for corner in CORNERS {
                    if let Some(corner) = corners.descriptor_mut(corner) {
                        self.point_descriptor(corner);
                    }
                }
            }
            if let Some(transform) = item.descriptor_mut("Trnf") {
                self.affine_descriptor(transform);
                continue;
            }
            if let Some(bounds) = item.descriptor_mut("keyOriginShapeBBox") {
                self.rect_descriptor(bounds);
            }
            for key in ["keyOriginLineStart", "keyOriginLineEnd"] {
                if let Some(point) = item.descriptor_mut(key) {
                    self.point_descriptor(point);
                }
            }
            if let Some(radii) = item.descriptor_mut("keyOriginRRectRadii") {
                self.radii(radii);
            }
        }
    }

    fn radii(self, radii: &mut Descriptor) {
        let scale = self.map.mean_scale();
        let mut moved = Vec::new();
        for (key, (x, y)) in RADII {
            let Some(mut value) = radii.get(key).cloned() else {
                continue;
            };
            if let Some(number) = value.number() {
                set_number(&mut value, number * scale);
            }
            let (x, y) = self.map.apply_vector(x, y);
            let target = RADII
                .iter()
                .find(|(_, corner)| corner.0 * x > 0.0 && corner.1 * y > 0.0)
                .map_or(key, |(target, _)| *target);
            moved.push((target, value));
        }
        for (key, value) in moved {
            radii.set(key, value);
        }
    }

    fn slices(self, descriptor: &mut Descriptor) {
        if let Some(bounds) = descriptor.descriptor_mut("bounds") {
            self.rect_descriptor(bounds);
        }
        if let Some(Value::List(slices)) = descriptor.get_mut("slices") {
            for slice in slices.iter_mut().filter_map(Value::as_descriptor_mut) {
                if let Some(bounds) = slice.descriptor_mut("bounds") {
                    self.rect_descriptor(bounds);
                }
            }
        }
    }

    fn layer_comps(self, descriptor: &mut Descriptor) {
        if let Some(point) = descriptor.descriptor_mut("origFXRefPoint") {
            self.point_descriptor(point);
        }
        self.offsets(descriptor, "layerSettings", "FXRefPoint");
    }

    fn frames(self, descriptor: &mut Descriptor) {
        self.offsets(descriptor, "LaSt", "FXRf");
    }

    fn offsets(self, descriptor: &mut Descriptor, list: &str, reference: &str) {
        let Some(Value::List(entries)) = descriptor.get_mut(list) else {
            return;
        };
        for entry in entries.iter_mut().filter_map(Value::as_descriptor_mut) {
            if let Some(offset) = entry.descriptor_mut("Ofst") {
                map_pair(offset, ["Hrzn", "Vrtc"], |x, y| self.map.apply_vector(x, y));
            }
            if let Some(point) = entry.descriptor_mut(reference) {
                self.point_descriptor(point);
            }
        }
    }

    fn gradient(self, descriptor: &mut Descriptor) {
        if descriptor.contains("Grad")
            && let Some(value) = descriptor.get_mut("Angl")
            && let Some(angle) = value.number()
        {
            set_number(value, self.angle(angle));
        }
    }

    fn point_descriptor(self, descriptor: &mut Descriptor) {
        map_pair(descriptor, ["Hrzn", "Vrtc"], |x, y| self.point(x, y));
    }

    fn rect_descriptor(self, descriptor: &mut Descriptor) {
        const KEYS: [&str; 4] = ["Left", "Top ", "Rght", "Btom"];
        let Some(numbers) = KEYS
            .iter()
            .map(|key| descriptor.number(key))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        let mapped = self.span([numbers[0], numbers[1], numbers[2], numbers[3]]);
        for (key, number) in KEYS.into_iter().zip(mapped) {
            if let Some(value) = descriptor.get_mut(key) {
                set_number(value, number);
            }
        }
    }

    fn affine_descriptor(self, descriptor: &mut Descriptor) {
        const KEYS: [&str; 6] = ["xx", "xy", "yx", "yy", "tx", "ty"];
        let Some(numbers) = KEYS
            .iter()
            .map(|key| descriptor.number(key))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        let mut array = [0.0; 6];
        array.copy_from_slice(&numbers);
        let mapped = Affine::from_array(array).then(self.map).to_array();
        for (key, number) in KEYS.into_iter().zip(mapped) {
            if let Some(value) = descriptor.get_mut(key) {
                set_number(value, number);
            }
        }
    }
}

fn map_pair(
    descriptor: &mut Descriptor,
    [first, second]: [&str; 2],
    map: impl Fn(f64, f64) -> (f64, f64),
) {
    let (Some(x), Some(y)) = (descriptor.number(first), descriptor.number(second)) else {
        return;
    };
    let (x, y) = map(x, y);
    for (key, number) in [(first, x), (second, y)] {
        if let Some(value) = descriptor.get_mut(key) {
            set_number(value, number);
        }
    }
}

fn scale_number(descriptor: &mut Descriptor, key: &str, factor: f64) {
    if factor == 1.0 {
        return;
    }
    if let Some(value) = descriptor.get_mut(key)
        && let Some(number) = value.number()
    {
        set_number(value, number * factor);
    }
}

fn set_number(value: &mut Value, number: f64) {
    match value {
        Value::Double(slot) | Value::UnitDouble(_, slot) => *slot = number,
        Value::Integer(slot) => {
            *slot = number
                .round()
                .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
        }
        Value::LargeInteger(slot) => *slot = number.round() as i64,
        Value::UnitFloats(_, values) if values.len() == 1 => values[0] = number,
        _ => {}
    }
}
