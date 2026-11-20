use rikka_physics_core::{Color, DebugDraw};
use rikka_physics_geometry::{Shape, ShapeGeometry};
use rikka_physics_math::{Aabb, Transform, Vec3, perpendicular};
use rikka_physics_rigid::{BodyId, BodyKind, JointId};

use super::World;

const AXIS_SCALE: f32 = 0.1;
const JOINT_SCALE: f32 = 0.05;
const POINT_SIZE: f32 = 0.02;
const NORMAL_LENGTH: f32 = 0.1;
const JOINT_GAP: f32 = 0.001;

impl World {
    pub fn debug_draw(&self, draw: &mut dyn DebugDraw, region: Aabb) {
        for &body in &self.bodies {
            let pose = self.pose(body).expect("world body is alive");
            let color = self.body_color(body);
            let shapes = self.rigid.body_shapes(body).expect("world body is alive");
            for shape in shapes {
                let geometry = self.rigid.shape(shape).expect("body shape is alive");
                if geometry.aabb(pose).overlaps(&region) {
                    draw_shape(draw, geometry, pose, color, region);
                }
            }
            let kind = self.rigid.body_kind(body).expect("world body is alive");
            if kind == BodyKind::Static {
                continue;
            }
            if region.contains_point(pose.p) {
                draw.draw_transform(pose, AXIS_SCALE);
            }
            self.draw_contacts(draw, body);
        }
        for &joint in &self.joints {
            self.draw_joint(draw, joint, region);
        }
        for (_, character) in self.characters.iter() {
            let mover = character.mover();
            let color = if character.is_grounded() {
                Color::GREEN
            } else {
                Color::YELLOW
            };
            if mover.bounds().overlaps(&region) {
                draw.draw_capsule(mover.center1, mover.center2, mover.radius, color);
            }
        }
    }

    fn draw_joint(&self, draw: &mut dyn DebugDraw, joint: JointId, region: Aabb) {
        let (Ok((body_a, body_b)), Ok((frame_a, frame_b))) = (
            self.rigid.joint_bodies(joint),
            self.rigid.joint_local_frames(joint),
        ) else {
            return;
        };
        let pose_a = self.pose(body_a).expect("joint body is alive");
        let pose_b = self.pose(body_b).expect("joint body is alive");
        let anchor_a = pose_a * frame_a;
        let anchor_b = pose_b * frame_b;
        if !region.contains_point(anchor_a.p) && !region.contains_point(anchor_b.p) {
            return;
        }
        let broken = self.rigid.is_joint_broken(joint).unwrap_or(false);
        let link = if broken { Color::GRAY } else { Color::CYAN };
        draw.draw_segment(pose_a.p, anchor_a.p, link);
        draw.draw_segment(pose_b.p, anchor_b.p, link);
        draw.draw_transform(anchor_a, JOINT_SCALE);
        if anchor_a.p.distance(anchor_b.p) > JOINT_GAP {
            draw.draw_segment(anchor_a.p, anchor_b.p, Color::RED);
        }
    }

    fn body_color(&self, body: BodyId) -> Color {
        match self.rigid.body_kind(body).expect("shape body is alive") {
            BodyKind::Static => Color::GRAY,
            BodyKind::Kinematic => Color::CYAN,
            BodyKind::Dynamic if self.rigid.is_awake(body).unwrap_or(false) => Color::ORANGE,
            BodyKind::Dynamic => Color::BLUE,
        }
    }

    fn draw_contacts(&self, draw: &mut dyn DebugDraw, body: BodyId) {
        let Ok(contacts) = self.rigid.body_contacts(body) else {
            return;
        };
        for contact in contacts {
            if self.rigid.shape_body(contact.shape_a).ok() != Some(body) {
                continue;
            }
            for manifold in &contact.manifolds {
                for point in manifold.points() {
                    draw.draw_point(point.point, POINT_SIZE, Color::YELLOW);
                    draw.draw_segment(
                        point.point,
                        point.point + manifold.normal * NORMAL_LENGTH,
                        Color::GREEN,
                    );
                }
            }
        }
    }
}

fn draw_shape(
    draw: &mut dyn DebugDraw,
    shape: &Shape,
    pose: Transform,
    color: Color,
    region: Aabb,
) {
    match shape {
        Shape::Sphere(sphere) => {
            draw.draw_sphere(
                Transform::new(pose.apply(sphere.center), pose.q),
                sphere.radius,
                color,
            );
        }
        Shape::Capsule(capsule) => draw.draw_tapered_capsule(
            pose.apply(capsule.center1),
            capsule.radius1,
            pose.apply(capsule.center2),
            capsule.radius2,
            color,
        ),
        Shape::Cuboid(cuboid) => {
            draw.draw_box(pose.mul(&cuboid.transform), cuboid.half_extents, color);
        }
        Shape::Cylinder(cylinder) => {
            let frame = pose.mul(&cylinder.transform);
            let axis = frame.rotate(Vec3::Y) * cylinder.half_height;
            let (top, bottom) = (frame.p + axis, frame.p - axis);
            draw.draw_circle(top, axis, cylinder.radius, color);
            draw.draw_circle(bottom, axis, cylinder.radius, color);
            let side = perpendicular(axis) * cylinder.radius;
            let other = axis.normalize_or_zero().cross(side);
            for offset in [side, -side, other, -other] {
                draw.draw_segment(top + offset, bottom + offset, color);
            }
        }
        Shape::Hull(hull) => {
            let points = hull.points();
            let edges = hull.edges();
            for (index, edge) in edges.iter().enumerate() {
                if index < edge.twin as usize {
                    let next = &edges[edge.next as usize];
                    draw.draw_segment(
                        pose.apply(points[edge.origin as usize]),
                        pose.apply(points[next.origin as usize]),
                        color,
                    );
                }
            }
        }
        Shape::Plane(plane) => {
            let normal = pose.rotate(plane.normal);
            let origin = pose.apply(plane.normal * plane.offset);
            let centre = region.center();
            let centre = centre - normal * normal.dot(centre - origin);
            let size = region.extent().max_element();
            let u = perpendicular(normal) * size;
            let v = normal.cross(u);
            draw.draw_polygon(
                &[
                    centre + u + v,
                    centre - u + v,
                    centre - u - v,
                    centre + u - v,
                ],
                color,
            );
        }
        Shape::Mesh(_) | Shape::HeightField(_) | Shape::Compound(_) => {
            draw.draw_aabb(shape.aabb(pose), color);
        }
    }
}
