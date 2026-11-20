use rikka_model_core::Endian;
use rikka_model_core::glam::{Vec2, Vec3, Vec4};
use rikka_polymesh::{
    Attribute, BEVEL_WEIGHT_EDGE, BEVEL_WEIGHT_VERT, CREASE_EDGE, CREASE_VERT, Domain as Element,
    HIDE_EDGE, HIDE_POLY, HIDE_VERT, Influence, Influences, MATERIAL_INDEX, Polymesh, SELECT_EDGE,
    SELECT_POLY, SELECT_VERT, SHARP_EDGE, SHARP_FACE, ShapeKey, UV_SEAM, UV_SELECT_EDGE,
    UV_SELECT_FACE, UV_SELECT_VERT, Values, VertexGroup, uv_pin_name,
};

use super::attributes::{Attributes, Data, Domain, Layer, legacy, srgb_to_linear};
use crate::file::{Record, Table, i32s};

const SELECT: i64 = 1;
const ME_SMOOTH: i64 = 1;
const ME_FACE_SEL: i64 = 1 << 1;
const ME_SEAM: i64 = 1 << 2;
const ME_HIDE: i64 = 1 << 4;
const ME_SHARP: i64 = 1 << 9;
const ME_AUTOSMOOTH: i64 = 1 << 5;
const ME_CDFLAG_VERT_BWEIGHT: i64 = 1;
const ME_CDFLAG_EDGE_BWEIGHT: i64 = 1 << 1;
const ME_CDFLAG_EDGE_CREASE: i64 = 1 << 2;
const MLOOPUV_EDGESEL: i64 = 1;
const MLOOPUV_VERTSEL: i64 = 1 << 1;
const MLOOPUV_PINNED: i64 = 1 << 2;
const CD_FLAG_COLOR_RENDER: i64 = 1 << 6;
const KEY_RELATIVE: i64 = 1;
const KEYBLOCK_MUTE: i64 = 1;
const OB_SHAPE_LOCK: i64 = 1;
const VERTEX_BYTES: usize = 12;
const EDGE_BYTES: usize = 8;
const FACE_BYTES: usize = 4;
const CORNER_BYTES: usize = 4;
const STRUCTURE: [&str; 5] = [
    "position",
    ".edge_verts",
    ".corner_vert",
    ".corner_edge",
    "custom_normal",
];
const LEGACY_SEAM: &str = ".uv_seam";
const LEGACY_UV_SELECT_VERT: &str = ".vs.";
const LEGACY_UV_SELECT_EDGE: &str = ".es.";

#[derive(Clone, Debug)]
pub(crate) enum Custom {
    Encoded(Vec<[i16; 2]>),
    Point(Vec<Vec3>),
    Face(Vec<Vec3>),
    Corner(Vec<Vec3>),
}

pub(crate) struct Source {
    pub(crate) mesh: Polymesh,
    pub(crate) custom: Option<Custom>,
    pub(crate) auto_smooth: Option<f32>,
    pub(crate) legacy_normals: bool,
    pub(crate) absolute_keys: bool,
    pub(crate) pinned_key: Option<usize>,
    pub(crate) repaired: bool,
}

struct Counts {
    points: usize,
    edges: usize,
    faces: usize,
    corners: usize,
}

#[derive(Default)]
struct Arrangement {
    faces: Option<Vec<usize>>,
    corners: Option<Vec<u32>>,
}

impl Arrangement {
    fn apply(&self, domain: Element, values: Values) -> Values {
        let order: Option<Vec<u32>> = match domain {
            Element::Face => self
                .faces
                .as_ref()
                .map(|faces| faces.iter().map(|&face| face as u32).collect()),
            Element::Corner => self.corners.clone(),
            _ => None,
        };
        match order {
            Some(order) => {
                let mut values = values;
                let needed = order.iter().max().map_or(0, |&index| index as usize + 1);
                if values.len() < needed {
                    values.resize(needed);
                }
                values.gather(&order);
                values
            }
            None => values,
        }
    }
}

pub(crate) fn read<'a>(mesh: Record<'a>, object: Option<Record<'a>>) -> Source {
    let blend = mesh.blend();
    let endian = blend.endian();
    let attributes = Attributes::of_mesh(mesh);
    let count = |name: &str, alias: &str, stride: usize| {
        (mesh
            .int(name)
            .or_else(|| mesh.int(alias))
            .unwrap_or(0)
            .max(0) as usize)
            .min(blend.size() / stride)
    };
    let counts = Counts {
        points: count("totvert", "verts_num", VERTEX_BYTES),
        edges: count("totedge", "edges_num", EDGE_BYTES),
        faces: count("totpoly", "faces_num", FACE_BYTES),
        corners: count("totloop", "corners_num", CORNER_BYTES),
    };
    let tessfaces = count("totface", "totface_legacy", FACE_BYTES);

    let mut polymesh = Polymesh::new();
    read_points(mesh, &attributes, &counts, endian, &mut polymesh);
    read_edges(mesh, &attributes, &counts, endian, &mut polymesh);
    let mut arrangement = Arrangement::default();
    let mut active_uv = None;
    match counts.faces > 0 || tessfaces == 0 {
        true => {
            arrangement = read_faces(mesh, &attributes, &counts, endian, &mut polymesh);
            read_corners(
                mesh,
                &attributes,
                &counts,
                endian,
                &arrangement,
                &mut polymesh,
            );
            active_uv = read_loop_uvs(&attributes, &counts, &arrangement, &mut polymesh);
        }
        false => read_tessfaces(mesh, &attributes, tessfaces, &mut polymesh),
    }
    read_generic(&attributes, &counts, endian, &arrangement, &mut polymesh);
    arrange_layers(mesh, &attributes, active_uv, &mut polymesh);
    let repaired = polymesh.sanitize();
    polymesh.ensure_edges();

    let flag = mesh.int("flag").unwrap_or(0);
    let modern_normals = blend.at_least(401, 2);
    let auto_smooth = (!modern_normals && flag & ME_AUTOSMOOTH != 0).then(|| {
        mesh.float("smoothresh")
            .or_else(|| mesh.float("smoothresh_legacy"))
            .unwrap_or(std::f32::consts::FRAC_PI_6)
    });
    let legacy_normals = !modern_normals && auto_smooth.is_none();
    let custom = match legacy_normals || repaired {
        true => None,
        false => custom_normals(&attributes, &polymesh, endian),
    };

    read_groups(mesh, object, &attributes, &mut polymesh);
    let keys = read_shape_keys(mesh, object, &mut polymesh);
    let locked = object
        .and_then(|object| object.int("shapeflag"))
        .is_some_and(|flag| flag & OB_SHAPE_LOCK != 0);
    let pinned_key = locked.then_some(keys.object_active).flatten();
    let absolute_keys = keys.absolute;
    Source {
        mesh: polymesh,
        custom,
        auto_smooth,
        legacy_normals,
        absolute_keys,
        pinned_key,
        repaired,
    }
}

fn insert(mesh: &mut Polymesh, name: &str, domain: Element, values: Values) {
    mesh.attributes.insert(Attribute::new(name, domain, values));
}

fn flags(values: &[i64], count: usize, mask: i64) -> Option<Values> {
    let flagged: Vec<bool> = values
        .iter()
        .take(count)
        .map(|flag| flag & mask != 0)
        .collect();
    flagged.contains(&true).then_some(Values::Bool(flagged))
}

fn bytes_to_unit(values: &[i64], count: usize) -> Option<Values> {
    let weights: Vec<f32> = values
        .iter()
        .take(count)
        .map(|&value| f32::from(value as u8) / 255.0)
        .collect();
    weights
        .iter()
        .any(|&weight| weight != 0.0)
        .then_some(Values::Float(weights))
}

fn read_points<'a>(
    mesh: Record<'a>,
    attributes: &Attributes<'a>,
    counts: &Counts,
    endian: Endian,
    polymesh: &mut Polymesh,
) {
    if let Some(values) = attributes
        .named(Domain::Point, "position")
        .and_then(|layer| layer.floats::<3>(counts.points, endian))
    {
        polymesh.positions = values.into_iter().map(Vec3::from_array).collect();
        return;
    }
    let Some(table) = attributes
        .first_legacy(Domain::Point, legacy::MVERT)
        .and_then(|layer| layer.table("MVert"))
        .or_else(|| mesh.table("mvert", "MVert"))
    else {
        return;
    };
    polymesh.positions = table
        .floats::<3>("co")
        .map(|values| {
            values
                .into_iter()
                .take(counts.points)
                .map(Vec3::from_array)
                .collect()
        })
        .unwrap_or_default();
    let points = polymesh.positions.len();
    let flag = table.ints("flag").unwrap_or_default();
    if let Some(values) = flags(&flag, points, SELECT) {
        insert(polymesh, SELECT_VERT, Element::Point, values);
    }
    if let Some(values) = flags(&flag, points, ME_HIDE) {
        insert(polymesh, HIDE_VERT, Element::Point, values);
    }
    if mesh.int("cd_flag").unwrap_or(0) & ME_CDFLAG_VERT_BWEIGHT != 0
        && let Some(values) = table
            .ints("bweight")
            .and_then(|weights| bytes_to_unit(&weights, points))
    {
        insert(polymesh, BEVEL_WEIGHT_VERT, Element::Point, values);
    }
}

fn read_edges<'a>(
    mesh: Record<'a>,
    attributes: &Attributes<'a>,
    counts: &Counts,
    endian: Endian,
    polymesh: &mut Polymesh,
) {
    let blend = mesh.blend();
    if let Some(pairs) = attributes
        .named(Domain::Edge, ".edge_verts")
        .and_then(|layer| layer.int_pairs(counts.edges, endian))
    {
        polymesh.edges = pairs
            .into_iter()
            .map(|pair| pair.map(|point| point.max(0) as u32))
            .collect();
        return;
    }
    let Some(table) = attributes
        .first_legacy(Domain::Edge, legacy::MEDGE)
        .and_then(|layer| layer.table("MEdge"))
        .or_else(|| mesh.table("medge", "MEdge"))
    else {
        return;
    };
    let first = table.ints("v1").unwrap_or_default();
    let second = table.ints("v2").unwrap_or_default();
    polymesh.edges = first
        .iter()
        .zip(&second)
        .take(counts.edges)
        .map(|(&a, &b)| [a.max(0) as u32, b.max(0) as u32])
        .collect();
    let edges = polymesh.edges.len();
    let flag = table.ints("flag").unwrap_or_default();
    for (name, mask) in [
        (SHARP_EDGE, ME_SHARP),
        (UV_SEAM, ME_SEAM),
        (SELECT_EDGE, SELECT),
        (HIDE_EDGE, ME_HIDE),
    ] {
        if let Some(values) = flags(&flag, edges, mask) {
            insert(polymesh, name, Element::Edge, values);
        }
    }
    let cd_flag = mesh.int("cd_flag").unwrap_or(0);
    let creased = !blend.at_least(265, 9) || cd_flag & ME_CDFLAG_EDGE_CREASE != 0;
    if creased
        && let Some(values) = table
            .ints("crease")
            .and_then(|creases| bytes_to_unit(&creases, edges))
    {
        insert(polymesh, CREASE_EDGE, Element::Edge, values);
    }
    if cd_flag & ME_CDFLAG_EDGE_BWEIGHT != 0
        && let Some(values) = table
            .ints("bweight")
            .and_then(|weights| bytes_to_unit(&weights, edges))
    {
        insert(polymesh, BEVEL_WEIGHT_EDGE, Element::Edge, values);
    }
}

fn read_faces<'a>(
    mesh: Record<'a>,
    attributes: &Attributes<'a>,
    counts: &Counts,
    endian: Endian,
    polymesh: &mut Polymesh,
) -> Arrangement {
    let offsets = mesh
        .raw("poly_offset_indices")
        .or_else(|| mesh.raw("face_offset_indices"))
        .map(|bytes| i32s(bytes, endian));
    if let Some(offsets) = offsets
        && offsets.len() > counts.faces
    {
        polymesh.face_offsets = offsets
            .iter()
            .take(counts.faces + 1)
            .map(|&offset| offset.clamp(0, counts.corners as i32) as u32)
            .collect();
        return Arrangement::default();
    }
    let Some(table) = attributes
        .first_legacy(Domain::Face, legacy::MPOLY)
        .and_then(|layer| layer.table("MPoly"))
        .or_else(|| mesh.table("mpoly", "MPoly"))
    else {
        return Arrangement::default();
    };
    let starts = table.ints("loopstart").unwrap_or_default();
    let sizes = table.ints("totloop").unwrap_or_default();
    let mut faces: Vec<(usize, i64, i64)> = starts
        .iter()
        .zip(&sizes)
        .take(counts.faces)
        .enumerate()
        .map(|(face, (&start, &size))| (face, start, size))
        .collect();
    let sorted = faces.windows(2).all(|pair| pair[0].1 <= pair[1].1);
    if !sorted {
        faces.sort_by_key(|&(_, start, _)| start);
    }
    let loops = counts.corners;
    let mut offsets = vec![0u32];
    let mut corners = Vec::with_capacity(loops);
    for &(_, start, size) in &faces {
        let first = start.clamp(0, loops as i64);
        let end = start.saturating_add(size).clamp(first, loops as i64);
        let room = loops.saturating_sub(corners.len()) as i64;
        corners.extend((first..end.min(first + room)).map(|corner| corner as u32));
        offsets.push(corners.len() as u32);
    }
    polymesh.face_offsets = offsets;
    let identity = corners
        .iter()
        .enumerate()
        .all(|(index, &corner)| index as u32 == corner);
    let arrangement = Arrangement {
        faces: (!sorted).then(|| faces.iter().map(|&(face, _, _)| face).collect()),
        corners: (!identity || corners.len() != loops).then_some(corners),
    };
    let face_count = faces.len();
    let source = |values: Vec<i64>| -> Vec<i64> {
        faces
            .iter()
            .map(|&(face, _, _)| values.get(face).copied().unwrap_or(0))
            .collect()
    };
    let materials: Vec<i32> = source(table.ints("mat_nr").unwrap_or_default())
        .into_iter()
        .map(|slot| slot.max(0) as i32)
        .collect();
    if materials.iter().any(|&slot| slot != 0) {
        insert(
            polymesh,
            MATERIAL_INDEX,
            Element::Face,
            Values::Int(materials),
        );
    }
    let flag = source(table.ints("flag").unwrap_or_default());
    let sharp: Vec<bool> = flag.iter().map(|flag| flag & ME_SMOOTH == 0).collect();
    if sharp.contains(&true) {
        insert(polymesh, SHARP_FACE, Element::Face, Values::Bool(sharp));
    }
    for (name, mask) in [(SELECT_POLY, ME_FACE_SEL), (HIDE_POLY, ME_HIDE)] {
        if let Some(values) = flags(&flag, face_count, mask) {
            insert(polymesh, name, Element::Face, values);
        }
    }
    arrangement
}

fn read_corners<'a>(
    mesh: Record<'a>,
    attributes: &Attributes<'a>,
    counts: &Counts,
    endian: Endian,
    arrangement: &Arrangement,
    polymesh: &mut Polymesh,
) {
    let named = |name: &str| {
        attributes
            .named(Domain::Corner, name)
            .and_then(|layer| layer.ints(counts.corners, endian))
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value.max(0))
                    .collect::<Vec<_>>()
            })
    };
    let table = attributes
        .first_legacy(Domain::Corner, legacy::MLOOP)
        .and_then(|layer| layer.table("MLoop"))
        .or_else(|| mesh.table("mloop", "MLoop"));
    let column = |name: &str| {
        table.and_then(|table| table.ints(name)).map(|values| {
            values
                .into_iter()
                .take(counts.corners)
                .map(|value| value.max(0) as i32)
                .collect::<Vec<_>>()
        })
    };
    let arrange = |values: Vec<i32>| -> Vec<u32> {
        match arrangement.apply(Element::Corner, Values::Int(values)) {
            Values::Int(values) => values.into_iter().map(|value| value as u32).collect(),
            _ => unreachable!("integer values stay integers"),
        }
    };
    polymesh.corner_verts = arrange(
        named(".corner_vert")
            .or_else(|| column("v"))
            .unwrap_or_default(),
    );
    polymesh.corner_edges = arrange(
        named(".corner_edge")
            .or_else(|| column("e"))
            .unwrap_or_default(),
    );
    let corners = polymesh.corner_verts.len();
    if polymesh.face_offsets.last().copied().unwrap_or(0) as usize > corners {
        polymesh
            .face_offsets
            .retain(|&offset| offset as usize <= corners);
    }
}

fn read_loop_uvs(
    attributes: &Attributes<'_>,
    counts: &Counts,
    arrangement: &Arrangement,
    polymesh: &mut Polymesh,
) -> Option<String> {
    let mut active = None;
    for layer in attributes.legacy(Domain::Corner, legacy::MLOOPUV) {
        let Some(table) = layer.table("MLoopUV") else {
            continue;
        };
        let Some(uvs) = table.floats::<2>("uv") else {
            continue;
        };
        let mut uvs: Vec<Vec2> = uvs.into_iter().map(Vec2::from_array).collect();
        uvs.resize(counts.corners, Vec2::ZERO);
        let uvs = arrangement.apply(Element::Corner, Values::Vec2(uvs));
        polymesh.add_uv_map(layer.name.clone(), uv_values(uvs));
        let flag: Vec<i64> = table.ints("flag").unwrap_or_default();
        let pinned = corner_flags(&flag, counts.corners, MLOOPUV_PINNED, arrangement);
        if let Some(pinned) = pinned {
            insert(polymesh, &uv_pin_name(&layer.name), Element::Corner, pinned);
        }
        if layer.active {
            active = Some(layer.name.clone());
            for (name, mask) in [
                (UV_SELECT_VERT, MLOOPUV_VERTSEL),
                (UV_SELECT_EDGE, MLOOPUV_EDGESEL),
            ] {
                if let Some(selected) = corner_flags(&flag, counts.corners, mask, arrangement) {
                    insert(polymesh, name, Element::Corner, selected);
                }
            }
        }
        if layer.render {
            polymesh.uv_maps.set_render(&layer.name);
        }
    }
    active
}

fn uv_values(values: Values) -> Vec<Vec2> {
    match values {
        Values::Vec2(values) => values,
        _ => unreachable!("UV values stay vectors"),
    }
}

fn corner_flags(
    flag: &[i64],
    corners: usize,
    mask: i64,
    arrangement: &Arrangement,
) -> Option<Values> {
    let mut flagged: Vec<bool> = flag
        .iter()
        .take(corners)
        .map(|flag| flag & mask != 0)
        .collect();
    if !flagged.contains(&true) {
        return None;
    }
    flagged.resize(corners, false);
    Some(arrangement.apply(Element::Corner, Values::Bool(flagged)))
}

fn element(domain: Domain) -> Option<Element> {
    match domain {
        Domain::Point => Some(Element::Point),
        Domain::Edge => Some(Element::Edge),
        Domain::Face => Some(Element::Face),
        Domain::Corner => Some(Element::Corner),
        Domain::Tessface => None,
    }
}

fn layer_values(layer: &Layer<'_>, count: usize, endian: Endian) -> Option<Values> {
    let scalars = |values: Vec<[f32; 1]>| values.into_iter().map(|[value]| value).collect();
    Some(match layer.data? {
        Data::Bool => Values::Bool(layer.bools(count)?),
        Data::Int8 | Data::Int32 => Values::Int(layer.ints(count, endian)?),
        Data::Float => Values::Float(scalars(layer.floats::<1>(count, endian)?)),
        Data::Float2 => Values::Vec2(
            layer
                .floats::<2>(count, endian)?
                .into_iter()
                .map(Vec2::from_array)
                .collect(),
        ),
        Data::Float3 => Values::Vec3(
            layer
                .floats::<3>(count, endian)?
                .into_iter()
                .map(Vec3::from_array)
                .collect(),
        ),
        Data::Float4 => Values::Color(
            layer
                .floats::<4>(count, endian)?
                .into_iter()
                .map(Vec4::from_array)
                .collect(),
        ),
        Data::ColorByte | Data::ColorFloat => Values::Color(
            layer
                .colors(count, endian)?
                .into_iter()
                .map(Vec4::from_array)
                .collect(),
        ),
        Data::Int16x2 | Data::Int32x2 => Values::Vec2(
            layer
                .int_pairs(count, endian)?
                .into_iter()
                .map(|[x, y]| Vec2::new(x as f32, y as f32))
                .collect(),
        ),
    })
}

fn generic_name(layer: &Layer<'_>, domain: Element) -> Option<String> {
    match (layer.legacy, domain) {
        (Some(legacy::BWEIGHT), Element::Point) => Some(BEVEL_WEIGHT_VERT.to_owned()),
        (Some(legacy::BWEIGHT), Element::Edge) => Some(BEVEL_WEIGHT_EDGE.to_owned()),
        (Some(legacy::CREASE), Element::Point) => Some(CREASE_VERT.to_owned()),
        (Some(legacy::CREASE), Element::Edge) => Some(CREASE_EDGE.to_owned()),
        (Some(legacy::BWEIGHT | legacy::CREASE | legacy::CUSTOMLOOPNORMAL), _) => None,
        _ if STRUCTURE.contains(&layer.name.as_str()) || layer.name.is_empty() => None,
        _ if layer.name == LEGACY_SEAM => Some(UV_SEAM.to_owned()),
        _ => Some(layer.name.clone()),
    }
}

fn read_generic(
    attributes: &Attributes<'_>,
    counts: &Counts,
    endian: Endian,
    arrangement: &Arrangement,
    polymesh: &mut Polymesh,
) {
    for layer in &attributes.layers {
        let Some(domain) = element(layer.domain) else {
            continue;
        };
        if layer.data.is_none() {
            continue;
        }
        let Some(name) = generic_name(layer, domain) else {
            continue;
        };
        let count = match domain {
            Element::Point => counts.points,
            Element::Edge => counts.edges,
            Element::Face => counts.faces,
            Element::Corner => counts.corners,
        };
        let Some(values) = layer_values(layer, count, endian) else {
            continue;
        };
        let values = arrangement.apply(domain, values);
        let values = match name.as_str() {
            MATERIAL_INDEX => values.convert(rikka_polymesh::DataType::Int),
            _ => values,
        };
        insert(polymesh, &name, domain, values);
    }
}

fn arrange_layers(
    mesh: Record<'_>,
    attributes: &Attributes<'_>,
    loop_active: Option<String>,
    polymesh: &mut Polymesh,
) {
    let mut active_uv = loop_active;
    let mut render_uv = polymesh.uv_maps.render_name().map(str::to_owned);
    for layer in &attributes.layers {
        if layer.domain != Domain::Corner
            || layer.data != Some(Data::Float2)
            || layer.name.starts_with('.')
        {
            continue;
        }
        if layer.legacy.is_some_and(|kind| kind != legacy::PROP_FLOAT2) {
            continue;
        }
        if polymesh
            .attributes
            .values::<Vec2>(&layer.name, Element::Corner)
            .is_none()
        {
            continue;
        }
        polymesh.uv_maps.push(layer.name.clone());
        if layer.active && active_uv.is_none() {
            active_uv = Some(layer.name.clone());
        }
        if layer.render && render_uv.is_none() {
            render_uv = Some(layer.name.clone());
        }
    }
    let uv_name = |field: &str| {
        mesh.string(field)
            .filter(|name| polymesh.uv_maps.contains(name))
    };
    let active_name = uv_name("active_uv_map_attribute").or(active_uv);
    let render_name = uv_name("default_uv_map_attribute").or(render_uv);
    if let Some(name) = active_name {
        polymesh.uv_maps.set_active(&name);
    }
    if let Some(name) = render_name {
        polymesh.uv_maps.set_render(&name);
    }
    adopt_legacy_uv_selection(polymesh);

    let mut flagged_render = None;
    let mut layer_render = None;
    let mut layer_active = None;
    for domain in [Domain::Corner, Domain::Point] {
        for layer in &attributes.layers {
            if layer.domain != domain
                || !matches!(layer.data, Some(Data::ColorByte | Data::ColorFloat))
                || layer.name.starts_with('.')
                || layer.name.is_empty()
            {
                continue;
            }
            let is_color = polymesh
                .attributes
                .get(&layer.name)
                .is_some_and(|attribute| matches!(attribute.values, Values::Color(_)));
            if !is_color {
                continue;
            }
            polymesh.color_attributes.push(layer.name.clone());
            if layer.flag & CD_FLAG_COLOR_RENDER != 0 && flagged_render.is_none() {
                flagged_render = Some(layer.name.clone());
            }
            if layer.render && layer_render.is_none() {
                layer_render = Some(layer.name.clone());
            }
            if layer.active && layer_active.is_none() {
                layer_active = Some(layer.name.clone());
            }
        }
    }
    let color_name = |field: &str| {
        mesh.string(field)
            .filter(|name| polymesh.color_attributes.contains(name))
    };
    let render_name = color_name("default_color_attribute")
        .or(flagged_render)
        .or(layer_render);
    let active_name = color_name("active_color_attribute").or(layer_active);
    if let Some(name) = render_name {
        polymesh.color_attributes.set_render(&name);
    }
    if let Some(name) = active_name {
        polymesh.color_attributes.set_active(&name);
    }
}

fn adopt_legacy_uv_selection(polymesh: &mut Polymesh) {
    if let Some(active) = polymesh.uv_maps.active_name().map(str::to_owned) {
        for (prefix, name) in [
            (LEGACY_UV_SELECT_VERT, UV_SELECT_VERT),
            (LEGACY_UV_SELECT_EDGE, UV_SELECT_EDGE),
        ] {
            let legacy_name = format!("{prefix}{active}");
            if !polymesh.attributes.contains(name)
                && let Some(mut attribute) = polymesh.attributes.remove(&legacy_name)
            {
                attribute.name = name.to_owned();
                polymesh.attributes.insert(attribute);
            }
        }
    }
    polymesh.attributes.retain(|attribute| {
        !(attribute.name.starts_with(LEGACY_UV_SELECT_VERT)
            || attribute.name.starts_with(LEGACY_UV_SELECT_EDGE))
    });
    if polymesh.attributes.contains(UV_SELECT_FACE) {
        return;
    }
    let Some(edges) = polymesh
        .attributes
        .values::<bool>(UV_SELECT_EDGE, Element::Corner)
        .map(<[bool]>::to_vec)
    else {
        return;
    };
    if edges.len() != polymesh.corner_count() {
        return;
    }
    let faces: Vec<bool> = (0..polymesh.face_count())
        .map(|face| {
            let range = polymesh.face(face);
            range.end <= edges.len() && edges[range].iter().all(|&selected| selected)
        })
        .collect();
    insert(polymesh, UV_SELECT_FACE, Element::Face, Values::Bool(faces));
}

fn read_tessfaces<'a>(
    mesh: Record<'a>,
    attributes: &Attributes<'a>,
    count: usize,
    polymesh: &mut Polymesh,
) {
    let Some(table) = attributes
        .first_legacy(Domain::Tessface, legacy::MFACE)
        .and_then(|layer| layer.table("MFace"))
        .or_else(|| mesh.table("mface", "MFace"))
    else {
        return;
    };
    let columns = ["v1", "v2", "v3", "v4"].map(|name| table.ints(name).unwrap_or_default());
    let corners: Vec<[i64; 4]> = (0..count.min(columns[0].len()))
        .map(|face| std::array::from_fn(|slot| columns[slot].get(face).copied().unwrap_or(0)))
        .collect();
    let sizes: Vec<usize> = corners
        .iter()
        .map(|face| match face[3] {
            0 => 3,
            _ => 4,
        })
        .collect();
    for (face, &size) in corners.iter().zip(&sizes) {
        polymesh
            .corner_verts
            .extend(face[..size].iter().map(|&point| point.max(0) as u32));
        polymesh
            .face_offsets
            .push(polymesh.corner_verts.len() as u32);
    }
    let faces = corners.len();
    let materials: Vec<i32> = table
        .ints("mat_nr")
        .unwrap_or_default()
        .iter()
        .take(faces)
        .map(|&slot| slot.max(0) as i32)
        .collect();
    if materials.iter().any(|&slot| slot != 0) {
        let mut materials = materials;
        materials.resize(faces, 0);
        insert(
            polymesh,
            MATERIAL_INDEX,
            Element::Face,
            Values::Int(materials),
        );
    }
    let mut flag = table.ints("flag").unwrap_or_default();
    flag.resize(faces, ME_SMOOTH);
    let sharp: Vec<bool> = flag.iter().map(|flag| flag & ME_SMOOTH == 0).collect();
    if sharp.contains(&true) {
        insert(polymesh, SHARP_FACE, Element::Face, Values::Bool(sharp));
    }
    for (name, mask) in [(SELECT_POLY, ME_FACE_SEL), (HIDE_POLY, ME_HIDE)] {
        if let Some(values) = flags(&flag, faces, mask) {
            insert(polymesh, name, Element::Face, values);
        }
    }
    let total = polymesh.corner_verts.len();

    for layer in attributes.legacy(Domain::Tessface, legacy::MTFACE) {
        let Some(uvs) = layer
            .table("MTFace")
            .and_then(|table| table.floats::<8>("uv"))
        else {
            continue;
        };
        let mut values = Vec::with_capacity(total);
        for (face, &size) in uvs.iter().zip(&sizes) {
            values.extend((0..size).map(|slot| Vec2::new(face[slot * 2], face[slot * 2 + 1])));
        }
        values.resize(total, Vec2::ZERO);
        polymesh.add_uv_map(layer.name.clone(), values);
        if layer.render {
            polymesh.uv_maps.set_render(&layer.name);
        }
        if layer.active {
            polymesh.uv_maps.set_active(&layer.name);
        }
    }

    for layer in attributes.legacy(Domain::Tessface, legacy::MCOL) {
        let Some(bytes) = layer.raw() else {
            continue;
        };
        let mut colors = Vec::with_capacity(total);
        for (face, &size) in bytes.as_chunks::<16>().0.iter().zip(&sizes) {
            for slot in 0..size {
                let [alpha, blue, green, red] = [
                    face[slot * 4],
                    face[slot * 4 + 1],
                    face[slot * 4 + 2],
                    face[slot * 4 + 3],
                ];
                colors.push(Vec4::new(
                    srgb_to_linear(red),
                    srgb_to_linear(green),
                    srgb_to_linear(blue),
                    f32::from(alpha) / 255.0,
                ));
            }
        }
        colors.resize(total, Vec4::ONE);
        polymesh.add_color_attribute(layer.name.clone(), Element::Corner, colors);
        if layer.render {
            polymesh.color_attributes.set_render(&layer.name);
        }
        if layer.active {
            polymesh.color_attributes.set_active(&layer.name);
        }
    }
}

fn custom_normals(
    attributes: &Attributes<'_>,
    polymesh: &Polymesh,
    endian: Endian,
) -> Option<Custom> {
    let corners = polymesh.corner_count();
    let encoded = |pairs: Vec<[i32; 2]>| {
        Custom::Encoded(
            pairs
                .into_iter()
                .map(|pair| {
                    pair.map(|value| value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16)
                })
                .collect(),
        )
    };
    for layer in attributes
        .layers
        .iter()
        .filter(|layer| layer.name == "custom_normal")
    {
        match (layer.domain, layer.data) {
            (Domain::Corner, Some(Data::Int16x2)) => {
                return layer.int_pairs(corners, endian).map(encoded);
            }
            (Domain::Corner, Some(Data::Float3)) => {
                return layer.floats::<3>(corners, endian).map(|values| {
                    Custom::Corner(values.into_iter().map(Vec3::from_array).collect())
                });
            }
            (Domain::Point, Some(Data::Float3)) => {
                return layer
                    .floats::<3>(polymesh.point_count(), endian)
                    .map(|values| {
                        Custom::Point(values.into_iter().map(Vec3::from_array).collect())
                    });
            }
            (Domain::Face, Some(Data::Float3)) => {
                return layer
                    .floats::<3>(polymesh.face_count(), endian)
                    .map(|values| {
                        Custom::Face(values.into_iter().map(Vec3::from_array).collect())
                    });
            }
            _ => {}
        }
    }
    attributes
        .first_legacy(Domain::Corner, legacy::CUSTOMLOOPNORMAL)
        .and_then(|layer| layer.int_pairs(corners, endian))
        .map(encoded)
}

fn group_names<'a>(mesh: Record<'a>, object: Option<Record<'a>>) -> Vec<VertexGroup> {
    let from_mesh = mesh.list("vertex_group_names");
    let groups = match from_mesh.is_empty() {
        true => object
            .map(|object| object.list("defbase"))
            .unwrap_or_default(),
        false => from_mesh,
    };
    groups
        .iter()
        .map(|group| VertexGroup {
            name: group.text("name").unwrap_or_default(),
            locked: group.int("flag").unwrap_or(0) & 1 != 0,
        })
        .collect()
}

fn read_groups<'a>(
    mesh: Record<'a>,
    object: Option<Record<'a>>,
    attributes: &Attributes<'a>,
    polymesh: &mut Polymesh,
) {
    let groups = group_names(mesh, object);
    if groups.is_empty() {
        return;
    }
    let active = mesh
        .int("vertex_group_active_index")
        .or_else(|| object.and_then(|object| object.int("actdef")))
        .and_then(|index| usize::try_from(index - 1).ok())
        .filter(|&index| index < groups.len());
    let group_count = groups.len();
    polymesh.vertex_groups.groups = groups;
    polymesh.vertex_groups.active = active.or(Some(0));
    let points = polymesh.point_count();
    let Some(table) = attributes
        .first_legacy(Domain::Point, legacy::MDEFORMVERT)
        .and_then(|layer| layer.table("MDeformVert"))
        .or_else(|| mesh.table("dvert", "MDeformVert"))
    else {
        return;
    };
    let mut weights: Vec<Influences> = Vec::with_capacity(points);
    for row in table.rows().take(points) {
        weights.push(influences(row, group_count));
    }
    weights.resize(points, Vec::new());
    polymesh.vertex_groups.weights = weights;
}

fn influences(row: Record<'_>, groups: usize) -> Influences {
    let total = row.int("totweight").unwrap_or(0).max(0) as usize;
    if total == 0 {
        return Vec::new();
    }
    let Some(entries): Option<Table<'_>> = row.table("dw", "MDeformWeight") else {
        return Vec::new();
    };
    entries
        .rows()
        .take(total)
        .filter_map(|entry| {
            let group = u32::try_from(entry.int("def_nr")?).ok()?;
            ((group as usize) < groups).then_some(Influence {
                group,
                weight: entry.float("weight")?,
            })
        })
        .collect()
}

#[derive(Default)]
struct KeyState {
    absolute: bool,
    object_active: Option<usize>,
}

fn read_shape_keys(
    mesh: Record<'_>,
    object: Option<Record<'_>>,
    polymesh: &mut Polymesh,
) -> KeyState {
    let Some(key) = mesh.id("key") else {
        return KeyState::default();
    };
    let endian = mesh.blend().endian();
    let points = polymesh.point_count();
    let blocks = key.list("block");
    if key.int("type").unwrap_or(KEY_RELATIVE) != KEY_RELATIVE {
        return KeyState {
            absolute: blocks.len() > 1,
            object_active: None,
        };
    }
    let data = |block: &Record<'_>| -> Option<Vec<Vec3>> {
        let values = crate::file::f32s(block.raw("data")?, endian);
        let positions: Vec<Vec3> = values
            .as_chunks::<3>()
            .0
            .iter()
            .take(points)
            .map(|point| Vec3::from_array(*point))
            .collect();
        (positions.len() == points).then_some(positions)
    };
    let mut kept: Vec<Option<usize>> = Vec::with_capacity(blocks.len());
    let mut keys: Vec<(ShapeKey, i64)> = Vec::new();
    for block in &blocks {
        let Some(positions) = data(block) else {
            kept.push(None);
            continue;
        };
        kept.push(Some(keys.len()));
        let group = block.text("vgroup").unwrap_or_default();
        keys.push((
            ShapeKey {
                name: block.text("name").unwrap_or_default(),
                positions,
                value: block.float("curval").unwrap_or(0.0),
                min: block.float("slidermin").unwrap_or(0.0),
                max: block.float("slidermax").unwrap_or(1.0),
                relative_to: 0,
                vertex_group: (!group.is_empty()).then_some(group),
                muted: block.int("flag").unwrap_or(0) & KEYBLOCK_MUTE != 0,
            },
            block.int("relative").unwrap_or(0),
        ));
    }
    if kept.first().copied().flatten() != Some(0) {
        return KeyState::default();
    }
    for (key, relative) in keys {
        let mut key = key;
        key.relative_to = usize::try_from(relative)
            .ok()
            .and_then(|relative| kept.get(relative).copied().flatten())
            .unwrap_or(0) as u32;
        polymesh.shape_keys.add(key);
    }
    polymesh.positions = polymesh.shape_keys.keys[0].positions.clone();
    let active = object
        .and_then(|object| object.int("shapenr"))
        .and_then(|number| usize::try_from(number - 1).ok())
        .and_then(|index| kept.get(index).copied().flatten());
    polymesh.shape_keys.active = active.or(Some(0));
    KeyState {
        absolute: false,
        object_active: active,
    }
}
