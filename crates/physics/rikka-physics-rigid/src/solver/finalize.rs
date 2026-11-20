use std::ops::Range;

use rikka_physics_collision::rotation_arc;
use rikka_physics_geometry::ShapeGeometry;
use rikka_physics_math::{Quat, Vec3};

use super::context::StepContext;
use crate::BodyId;
use crate::BodyMoveEvent;
use crate::body::{BULLET, BodyKind, NONE, SLEEP_ENABLED};

pub(crate) fn finalize_bodies(ctx: &StepContext<'_>, range: Range<usize>, worker: u32) {
    let f = &ctx.finalize;
    let mut output = f.output.local(worker);
    for local in range {
        let state = unsafe { ctx.integrate.states.state(local as u32 + 1) };
        let pose = unsafe { ctx.poses.get(local) };
        let motion = unsafe { ctx.integrate.motion.get(local) };
        let body = f.body_ids[local];
        let record = unsafe { f.bodies.get(body as usize) };
        let v = state.v;
        let w = state.w;
        let dp = state.dp;
        let dq = state.dq;

        let inverse = pose.transform.q.conjugate();
        let max_velocity = v.length() + rotation_arc(inverse * w, pose.reach).length();
        let max_delta = dp.length()
            + 2.0 * rotation_arc(inverse * Vec3::new(dq.x, dq.y, dq.z), pose.reach).length();
        let mut rotation = (dq * pose.transform.q).normalize();
        let mut center = pose.center + dp;
        if let Some(target) = record.target.take() {
            rotation = target.q;
            center = target.apply(pose.local_center);
        }
        pose.set_center(center, rotation);
        state.dp = Vec3::ZERO;
        state.dq = Quat::IDENTITY;
        motion.force = Vec3::ZERO;
        motion.torque = Vec3::ZERO;
        motion.refresh_world_inertia(rotation);

        let extent = pose.extent;
        let sleep_velocity = max_velocity.max(0.5 * f.inv_dt * max_delta);

        let sleepy = f.sleeping
            && record.flags & SLEEP_ENABLED != 0
            && sleep_velocity <= record.sleep_threshold;
        if sleepy {
            record.sleep_time += f.dt;
        } else {
            record.sleep_time = 0.0;
        }
        let dynamic = record.kind == BodyKind::Dynamic;
        let fast = dynamic && !sleepy && max_delta.max(max_velocity * f.dt) > 0.5 * pose.min_extent;
        if fast {
            if record.flags & BULLET != 0 {
                output.bullets.push(body);
            } else {
                output.fast.push(body);
            }
        } else {
            pose.center0 = pose.center;
            pose.rotation0 = rotation;
        }
        if record.island != NONE && record.sleep_time < f.time_to_sleep {
            output.awake_islands.set_growing(record.island as usize);
        }
        if !dynamic && !sleepy {
            output.moving_kinematic.push(body);
        }

        let travel = v * f.dt;
        let reach = w.length() * extent * f.dt + f.slop;
        let unclamped = travel.length() + reach;
        let speculative = unclamped.min(f.speculative_cap);
        let scale = speculative / unclamped;
        let frame = unsafe { ctx.frames.get(body as usize) };
        frame.transform = pose.transform;
        frame.center = pose.center;
        frame.reach = pose.reach;
        frame.speculative = speculative;
        frame.motion = travel * scale;
        frame.turn = w * (f.dt * scale);
        frame.sweep = reach * scale;
        frame.fast = fast;

        let mut shape = record.shape_head;
        while shape != NONE {
            let collider = &f.colliders[shape as usize];
            let raw = collider.shape.aabb(pose.transform);
            let bounds = frame.swept(raw);
            let fat = unsafe { f.fat.get(shape as usize) };
            if !fat.contains(&bounds) {
                *fat = bounds.inflated(f.tolerance.aabb_margin(raw.size().max_element()));
                output.enlarged.push(shape);
            }
            shape = f.shapes[shape as usize].next;
        }

        record.move_event = local as u32;
        *unsafe { f.moves.get(local) } = BodyMoveEvent {
            body: BodyId {
                index: body,
                generation: f.generations[body as usize],
            },
            transform: pose.transform,
            user_data: record.user_data,
            fell_asleep: false,
        };
    }
}
