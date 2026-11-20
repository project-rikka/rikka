use rikka_physics_math::{Transform, Vec3};

use crate::convex::distance::{
    DistanceInput, ShapeCastPairInput, SimplexCache, closest_features, shape_cast_pair,
    shape_distance,
};
use crate::convex::support::Support;
use crate::{CastHit, Error, Result, ShapeCastInput, ShapeProxy, Tolerance};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointProjection {
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}

pub fn project_point<S: Support + ?Sized>(shape: &S, point: Vec3) -> Option<PointProjection> {
    let target = [point];
    let proxy = ShapeProxy::new(&target, 0.0);
    let mut cache = SimplexCache::EMPTY;
    let features = closest_features(shape, &proxy, Transform::IDENTITY, &mut cache)?;
    Some(PointProjection {
        point: features.point_a,
        normal: features.normal,
        distance: features.separation,
    })
}

pub(crate) fn shape_cast<S: Support + ?Sized>(
    shape: &S,
    cast: &ShapeCastInput<'_>,
) -> Option<CastHit> {
    let mut input =
        ShapeCastPairInput::new(shape, &cast.proxy, Transform::IDENTITY, cast.translation);
    input.max_fraction = cast.max_fraction;
    input.can_encroach = cast.can_encroach;
    shape_cast_pair(&input)
}

pub(crate) fn overlaps<S: Support + ?Sized>(
    shape: &S,
    transform: Transform,
    proxy: &ShapeProxy<'_>,
) -> bool {
    let input = DistanceInput::new(shape, proxy, transform.inverse()).with_radii();
    let mut cache = SimplexCache::EMPTY;
    let output = shape_distance(&input, &mut cache);
    output.distance < overlap_slop()
}

#[inline]
pub(crate) fn overlap_slop() -> f32 {
    0.1 * Tolerance::default().linear_slop()
}

pub(crate) fn check_positive(what: &'static str, value: f32) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(Error::InvalidDimension { what, value })
    }
}

pub(crate) fn check_non_negative(what: &'static str, value: f32) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(Error::NegativeRadius { what, value })
    }
}

pub(crate) fn check_point(what: &'static str, point: Vec3) -> Result<()> {
    if point.is_finite() {
        Ok(())
    } else {
        Err(Error::NonFinite { what })
    }
}

pub(crate) fn check_transform(what: &'static str, transform: Transform) -> Result<()> {
    check_point(what, transform.p)?;
    let length = transform.q.length();
    if transform.q.is_finite() && (length - 1.0).abs() < 1.0e-3 {
        Ok(())
    } else {
        Err(Error::UnnormalizedRotation { what, length })
    }
}
