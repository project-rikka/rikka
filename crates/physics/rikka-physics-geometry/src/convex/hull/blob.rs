use bytemuck::{Pod, Zeroable};
use rikka_physics_core::{Blob, BlobCache, BlobWriter, Section};
use rikka_physics_math::{Aabb, Mat3, Plane, Vec3};

use super::bulk::bulk_properties;
use super::quickhull::{Topology, quickhull};
use super::view::{EDGE_ROWS, HullEdge, Polyhedron, soa_stride};
use crate::convex::queries;
use crate::{Error, Result, ShapeProxy};

pub const MAX_HULL_VERTICES: usize = 255;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HullHeader {
    pub bounds: Aabb,
    pub center: Vec3,
    pub volume: f32,
    pub surface_area: f32,
    pub inner_radius: f32,
    pub unit_inertia: Mat3,
    pub box_axes: [Vec3; 3],
    pub points: Section<Vec3>,
    pub vertex_edges: Section<u16>,
    pub edges: Section<HullEdge>,
    pub faces: Section<u16>,
    pub planes: Section<Plane>,
    pub soa_points: Section<f32>,
    pub soa_normals: Section<f32>,
    pub edge_normals: Section<f32>,
    pub face_distances: Section<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConvexHull {
    blob: Blob<HullHeader>,
    radius: f32,
}

impl ConvexHull {
    pub fn new(points: &[Vec3]) -> Result<Self> {
        Self::with_max_vertices(points, MAX_HULL_VERTICES)
    }

    pub fn with_max_vertices(points: &[Vec3], max_vertices: usize) -> Result<Self> {
        if max_vertices > MAX_HULL_VERTICES {
            return Err(Error::HullTooLarge {
                requested: max_vertices,
            });
        }
        for &point in points {
            queries::check_point("hull point", point)?;
        }
        let degenerate = || Error::DegenerateHull {
            points: points.len(),
        };
        let topology = quickhull(points, max_vertices).ok_or_else(degenerate)?;
        Self::from_topology(topology).ok_or_else(degenerate)
    }

    pub(crate) fn from_topology(topology: Topology) -> Option<Self> {
        let bulk = bulk_properties(
            &topology.points,
            &topology.edges,
            &topology.faces,
            &topology.planes,
        )?;
        let soa_points = soa(topology.points.iter().copied(), topology.points[0]);
        let soa_normals = soa(topology.planes.iter().map(|plane| plane.normal), Vec3::ZERO);
        let face_distances = distances(&topology.planes, bulk.center);
        let edge_normals = edge_rows(&topology.edges, &topology.planes, &face_distances);

        let mut writer = BlobWriter::<HullHeader>::new();
        let points = writer.push(&topology.points);
        let vertex_edges = writer.push(&topology.vertex_edges);
        let edges = writer.push(&topology.edges);
        let faces = writer.push(&topology.faces);
        let planes = writer.push(&topology.planes);
        let soa_points = writer.push(&soa_points);
        let soa_normals = writer.push(&soa_normals);
        let edge_normals = writer.push(&edge_normals);
        let box_axes = box_axes(&topology.planes, &face_distances);
        let face_distances = writer.push(&face_distances);
        let blob = writer.finish(HullHeader {
            bounds: Aabb::from_points(topology.points.iter().copied()),
            center: bulk.center,
            volume: bulk.volume,
            surface_area: bulk.surface_area,
            inner_radius: bulk.inner_radius,
            unit_inertia: bulk.inertia,
            box_axes,
            points,
            vertex_edges,
            edges,
            faces,
            planes,
            soa_points,
            soa_normals,
            edge_normals,
            face_distances,
        });
        Some(Self::wrap(blob))
    }

    fn wrap(blob: Blob<HullHeader>) -> Self {
        Self { blob, radius: 0.0 }
    }

    pub fn from_blob(blob: Blob<HullHeader>) -> Result<Self> {
        let header = *blob.header();
        let points = blob.try_section(header.points)?;
        let vertex_edges = blob.try_section(header.vertex_edges)?;
        let edges = blob.try_section(header.edges)?;
        let faces = blob.try_section(header.faces)?;
        let planes = blob.try_section(header.planes)?;
        let soa_points = blob.try_section(header.soa_points)?;
        let soa_normals = blob.try_section(header.soa_normals)?;
        let edge_normals = blob.try_section(header.edge_normals)?;
        let face_distances = blob.try_section(header.face_distances)?;
        let vertex_count = points.len();
        if vertex_count > MAX_HULL_VERTICES {
            return Err(Error::HullTooLarge {
                requested: vertex_count,
            });
        }
        let consistent = vertex_count >= 4
            && vertex_edges.len() == vertex_count
            && faces.len() == planes.len()
            && faces.len() >= 4
            && faces.len() <= 2 * vertex_count - 4
            && edges.len().is_multiple_of(2)
            && vertex_count + faces.len() == edges.len() / 2 + 2
            && vertex_edges
                .iter()
                .all(|&edge| (edge as usize) < edges.len())
            && faces.iter().all(|&edge| (edge as usize) < edges.len())
            && edges.iter().enumerate().all(|(index, edge)| {
                (edge.next as usize) < edges.len()
                    && edge.twin as usize == index ^ 1
                    && (edge.face as usize) < faces.len()
                    && (edge.origin as usize) < vertex_count
            })
            && header.bounds == Aabb::from_points(points.iter().copied())
            && soa_points == soa(points.iter().copied(), points[0])
            && soa_normals == soa(planes.iter().map(|plane| plane.normal), Vec3::ZERO)
            && face_distances == distances(planes, header.center)
            && edge_normals == edge_rows(edges, planes, face_distances)
            && header.box_axes == box_axes(planes, face_distances);
        if consistent {
            Ok(Self::wrap(blob))
        } else {
            Err(Error::CorruptShape {
                shape: "convex hull",
            })
        }
    }

    #[inline]
    #[must_use]
    pub fn rounded(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    #[inline]
    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn validate(&self) -> Result<()> {
        queries::check_non_negative("hull radius", self.radius)
    }

    #[inline]
    pub fn blob(&self) -> &Blob<HullHeader> {
        &self.blob
    }

    #[inline]
    #[must_use]
    pub fn interned(self, cache: &BlobCache<HullHeader>) -> Self {
        Self {
            blob: cache.intern(self.blob),
            radius: self.radius,
        }
    }

    #[inline]
    pub fn header(&self) -> &HullHeader {
        self.blob.header()
    }

    #[inline]
    pub fn points(&self) -> &[Vec3] {
        section(self.blob.bytes(), self.header().points)
    }

    #[inline]
    pub fn edges(&self) -> &[HullEdge] {
        section(self.blob.bytes(), self.header().edges)
    }

    #[inline]
    pub fn planes(&self) -> &[Plane] {
        section(self.blob.bytes(), self.header().planes)
    }

    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.header().points.len()
    }

    #[inline]
    pub fn face_count(&self) -> usize {
        self.header().planes.len()
    }

    #[inline]
    pub fn volume(&self) -> f32 {
        self.header().volume
    }

    #[inline]
    pub fn surface_area(&self) -> f32 {
        self.header().surface_area
    }

    #[inline]
    pub fn center(&self) -> Vec3 {
        self.header().center
    }

    #[inline]
    pub fn inner_radius(&self) -> f32 {
        self.header().inner_radius
    }

    #[inline]
    pub fn bounds(&self) -> Aabb {
        self.header().bounds
    }

    #[inline]
    pub fn content_hash(&self) -> u64 {
        self.blob.content_hash()
    }

    #[inline]
    pub fn proxy(&self) -> ShapeProxy<'_> {
        ShapeProxy::new(self.points(), self.radius)
    }

    pub(crate) fn polyhedron(&self) -> Polyhedron<'_> {
        let header = self.blob.header();
        let bytes = self.blob.bytes();
        Polyhedron {
            points: section(bytes, header.points),
            vertex_edges: section(bytes, header.vertex_edges),
            edges: section(bytes, header.edges),
            faces: section(bytes, header.faces),
            planes: section(bytes, header.planes),
            soa_points: section(bytes, header.soa_points),
            soa_normals: section(bytes, header.soa_normals),
            edge_normals: section(bytes, header.edge_normals),
            face_distances: section(bytes, header.face_distances),
            center: header.center,
            inner_radius: header.inner_radius,
            box_axes: header.box_axes,
        }
    }

    pub(crate) fn topology(&self) -> Topology {
        let view = self.polyhedron();
        Topology {
            points: view.points.to_vec(),
            vertex_edges: view.vertex_edges.to_vec(),
            edges: view.edges.to_vec(),
            faces: view.faces.to_vec(),
            planes: view.planes.to_vec(),
        }
    }
}

fn soa(values: impl ExactSizeIterator<Item = Vec3> + Clone, padding: Vec3) -> Vec<f32> {
    let count = values.len();
    let stride = soa_stride(count);
    let mut out = vec![0.0; 3 * stride];
    for (index, value) in values
        .chain(std::iter::repeat_n(padding, stride - count))
        .enumerate()
    {
        out[index] = value.x;
        out[stride + index] = value.y;
        out[2 * stride + index] = value.z;
    }
    out
}

fn distances(planes: &[Plane], center: Vec3) -> Vec<f32> {
    let mut out = vec![0.0; soa_stride(planes.len())];
    for (slot, plane) in out.iter_mut().zip(planes) {
        *slot = -plane.separation(center);
    }
    out
}

fn box_axes(planes: &[Plane], distances: &[f32]) -> [Vec3; 3] {
    const SQUARE: f32 = 1.0e-6;
    let mut axes = [Vec3::ZERO; 3];
    if planes.len() != 6 {
        return axes;
    }
    let mut found = 0;
    for (index, plane) in planes.iter().enumerate() {
        let Some(opposite) = planes
            .iter()
            .position(|other| other.normal.dot(plane.normal) < SQUARE - 1.0)
        else {
            return [Vec3::ZERO; 3];
        };
        if opposite < index {
            continue;
        }
        let extent = distances[index].min(distances[opposite]);
        if found == 3
            || axes[..found]
                .iter()
                .any(|axis| axis.normalize().dot(plane.normal).abs() > SQUARE)
        {
            return [Vec3::ZERO; 3];
        }
        axes[found] = extent * plane.normal;
        found += 1;
    }
    if found == 3 { axes } else { [Vec3::ZERO; 3] }
}

#[inline(always)]
fn section<T: Pod>(bytes: &[u8], section: Section<T>) -> &[T] {
    let start = section.offset as usize;
    bytemuck::cast_slice(&bytes[start..start + section.len as usize * size_of::<T>()])
}

fn edge_rows(edges: &[HullEdge], planes: &[Plane], distances: &[f32]) -> Vec<f32> {
    let stride = soa_stride(edges.len() / 2);
    let mut out = vec![0.0; EDGE_ROWS * stride];
    for (pair, [edge, twin]) in edges.as_chunks::<2>().0.iter().enumerate() {
        let [first, second] = [edge.face, twin.face].map(usize::from);
        let (u, v) = (planes[first].normal, planes[second].normal);
        let values = [
            u.x,
            u.y,
            u.z,
            v.x,
            v.y,
            v.z,
            u.dot(v),
            distances[first].min(distances[second]),
        ];
        for (row, value) in values.into_iter().enumerate() {
            out[row * stride + pair] = value;
        }
    }
    out
}
