use rikka_physics_math::{Aabb, Transform, Vec3, perpendicular};

use crate::Color;

const CIRCLE_SEGMENTS: u32 = 24;

pub trait DebugDraw {
    fn draw_segment(&mut self, a: Vec3, b: Vec3, color: Color);

    fn draw_point(&mut self, point: Vec3, size: f32, color: Color);

    fn draw_triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color) {
        self.draw_polygon(&[a, b, c], color);
    }

    fn draw_polygon(&mut self, vertices: &[Vec3], color: Color) {
        let Some(&last) = vertices.last() else {
            return;
        };
        let mut previous = last;
        for &vertex in vertices {
            self.draw_segment(previous, vertex, color);
            previous = vertex;
        }
    }

    fn draw_circle(&mut self, center: Vec3, normal: Vec3, radius: f32, color: Color) {
        let u = perpendicular(normal) * radius;
        let v = normal.normalize_or_zero().cross(u);
        draw_arc(self, center, u, v, 1.0, color);
    }

    fn draw_sphere(&mut self, transform: Transform, radius: f32, color: Color) {
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            self.draw_circle(transform.p, transform.rotate(axis), radius, color);
        }
    }

    fn draw_capsule(&mut self, a: Vec3, b: Vec3, radius: f32, color: Color) {
        self.draw_tapered_capsule(a, radius, b, radius, color);
    }

    fn draw_tapered_capsule(
        &mut self,
        a: Vec3,
        radius_a: f32,
        b: Vec3,
        radius_b: f32,
        color: Color,
    ) {
        let length = a.distance(b);
        if length <= (radius_a - radius_b).abs() {
            let (centre, radius) = if radius_a >= radius_b {
                (a, radius_a)
            } else {
                (b, radius_b)
            };
            self.draw_sphere(Transform::from_translation(centre), radius, color);
            return;
        }
        let axis = (b - a).normalize_or(Vec3::Y);
        let sine = if length > 0.0 {
            (radius_a - radius_b) / length
        } else {
            0.0
        };
        let cosine = (1.0 - sine * sine).max(0.0).sqrt();
        let cap = sine.clamp(-1.0, 1.0).acos() / std::f32::consts::PI;
        let u = perpendicular(axis);
        let v = axis.cross(u);
        for side in [u, v] {
            for flank in [side, -side] {
                let normal = axis * sine + flank * cosine;
                self.draw_segment(a + normal * radius_a, b + normal * radius_b, color);
            }
            let normal = axis * sine + side * cosine;
            let toward = axis * cosine - side * sine;
            draw_arc(self, b, normal * radius_b, toward * radius_b, cap, color);
            draw_arc(
                self,
                a,
                normal * radius_a,
                -toward * radius_a,
                1.0 - cap,
                color,
            );
        }
        self.draw_circle(a + axis * (radius_a * sine), axis, radius_a * cosine, color);
        self.draw_circle(b + axis * (radius_b * sine), axis, radius_b * cosine, color);
    }

    fn draw_box(&mut self, transform: Transform, half_extents: Vec3, color: Color) {
        let corners = box_corners(Vec3::ZERO, half_extents).map(|corner| transform.apply(corner));
        draw_box_edges(self, &corners, color);
    }

    fn draw_aabb(&mut self, aabb: Aabb, color: Color) {
        let corners = box_corners(aabb.center(), aabb.extent());
        draw_box_edges(self, &corners, color);
    }

    fn draw_transform(&mut self, transform: Transform, scale: f32) {
        for (axis, color) in [
            (Vec3::X, Color::RED),
            (Vec3::Y, Color::GREEN),
            (Vec3::Z, Color::BLUE),
        ] {
            self.draw_segment(
                transform.p,
                transform.p + transform.rotate(axis) * scale,
                color,
            );
        }
    }
}

fn draw_arc<D: DebugDraw + ?Sized>(
    draw: &mut D,
    center: Vec3,
    u: Vec3,
    v: Vec3,
    turns: f32,
    color: Color,
) {
    let segments = (CIRCLE_SEGMENTS as f32 * turns).ceil().max(1.0) as u32;
    let step = std::f32::consts::TAU * turns / segments as f32;
    let mut previous = center + u;
    for index in 1..=segments {
        let (sin, cos) = (step * index as f32).sin_cos();
        let point = center + u * cos + v * sin;
        draw.draw_segment(previous, point, color);
        previous = point;
    }
}

fn box_corners(center: Vec3, half: Vec3) -> [Vec3; 8] {
    std::array::from_fn(|corner| {
        let sign = |bit: usize| if corner & bit == 0 { -1.0 } else { 1.0 };
        center + Vec3::new(sign(1) * half.x, sign(2) * half.y, sign(4) * half.z)
    })
}

fn draw_box_edges<D: DebugDraw + ?Sized>(draw: &mut D, corners: &[Vec3; 8], color: Color) {
    for from in 0..8 {
        for bit in [1, 2, 4] {
            if from & bit == 0 {
                draw.draw_segment(corners[from], corners[from | bit], color);
            }
        }
    }
}
