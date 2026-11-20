use rikka_physics_math::{Plane, Vec3, Vec4};

pub(crate) const MAX_CLIP_POINTS: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Owner {
    #[default]
    A,
    B,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FeaturePair(u32);

impl FeaturePair {
    pub(crate) const SINGLE: Self = Self::new(Owner::A, 0, Owner::A, 0);

    #[inline(always)]
    pub(crate) const fn new(owner1: Owner, index1: u16, owner2: Owner, index2: u16) -> Self {
        Self(
            (owner1.bit() << 31)
                | ((index1 as u32 & 0x7fff) << 16)
                | (owner2.bit() << 15)
                | (index2 as u32 & 0x7fff),
        )
    }

    #[inline]
    pub(crate) fn flip_id(id: u32) -> u32 {
        id.rotate_left(16) ^ 0x8000_8000
    }

    #[inline(always)]
    pub(crate) fn id(self) -> u32 {
        self.0
    }

    #[inline]
    fn entering(self, edge: u16) -> Self {
        Self((self.0 & 0xffff_0000) | (u32::from(edge) & 0x7fff))
    }

    #[inline]
    fn leaving(self, edge: u16) -> Self {
        Self((self.0 & 0x0000_ffff) | ((u32::from(edge) & 0x7fff) << 16))
    }
}

impl Owner {
    #[inline(always)]
    const fn bit(self) -> u32 {
        match self {
            Self::A => 0,
            Self::B => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ClipVertex {
    pub(crate) position: Vec4,
    pub(crate) separation: f32,
    pub(crate) pair: FeaturePair,
}

impl ClipVertex {
    #[inline]
    pub(crate) fn new(position: Vec3, pair: FeaturePair) -> Self {
        Self {
            position: position.extend(0.0),
            separation: 0.0,
            pair,
        }
    }

    #[inline(always)]
    pub(crate) fn point(&self) -> Vec3 {
        self.position.truncate()
    }
}

pub(crate) fn clip_segment(segment: &mut [ClipVertex; 2], plane: Plane) -> usize {
    let [vertex1, vertex2] = *segment;
    let distance1 = plane.separation(vertex1.point());
    let distance2 = plane.separation(vertex2.point());
    let mut count = 0;
    if distance1 <= 0.0 {
        segment[count] = vertex1;
        count += 1;
    }
    if distance2 <= 0.0 {
        segment[count] = vertex2;
        count += 1;
    }
    if distance1 * distance2 < 0.0 {
        let t = distance1 / (distance1 - distance2);
        segment[count].position = vertex1.position.lerp(vertex2.position, t);
        segment[count].pair = if distance1 > 0.0 {
            vertex1.pair
        } else {
            vertex2.pair
        };
        count += 1;
    }
    count
}

pub(crate) fn clip_polygon<const N: usize>(
    output: &mut [ClipVertex; N],
    polygon: &[ClipVertex],
    clip_plane: Plane,
    edge: u16,
    reference: Plane,
) -> Option<usize> {
    let clip_normal = clip_plane.normal.extend(0.0);
    let reference_normal = reference.normal.extend(0.0);
    if polygon.is_empty() {
        return Some(0);
    }
    let mut distances = [0.0f32; N];
    let mut outside = false;
    for (distance, vertex) in distances.iter_mut().zip(polygon) {
        *distance = clip_normal.dot(vertex.position) - clip_plane.offset;
        outside |= *distance > 0.0;
    }
    if !outside {
        return None;
    }
    let separation = |position: Vec4| reference_normal.dot(position) - reference.offset;
    let mut count = 0;
    let last = polygon.len() - 1;
    let mut vertex1 = polygon[last];
    let mut distance1 = distances[last];
    for (&vertex2, &distance2) in polygon.iter().zip(&distances) {
        if distance1 <= 0.0 && distance2 <= 0.0 {
            if count < N {
                output[count] = vertex2;
                count += 1;
            }
        } else if distance1 <= 0.0 && distance2 > 0.0 {
            let fraction = distance1 / (distance1 - distance2);
            let position = vertex1.position + fraction * (vertex2.position - vertex1.position);
            if count < N {
                output[count] = ClipVertex {
                    position,
                    separation: separation(position),
                    pair: vertex2.pair.entering(edge),
                };
                count += 1;
            }
        } else if distance2 <= 0.0 && distance1 > 0.0 {
            let fraction = distance1 / (distance1 - distance2);
            let position = vertex1.position + fraction * (vertex2.position - vertex1.position);
            if count + 1 < N {
                output[count] = ClipVertex {
                    position,
                    separation: separation(position),
                    pair: vertex1.pair.leaving(edge),
                };
                output[count + 1] = vertex2;
                count += 2;
            }
        }
        vertex1 = vertex2;
        distance1 = distance2;
    }
    Some(count)
}
