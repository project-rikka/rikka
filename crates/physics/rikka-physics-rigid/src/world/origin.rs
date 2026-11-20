use rikka_physics_math::Vec3;

use super::RigidWorld;

impl RigidWorld {
    pub fn shift_origin(&mut self, offset: Vec3) {
        for set in &mut self.sets.sets {
            for pose in &mut set.poses {
                pose.transform.p -= offset;
                pose.center -= offset;
                pose.center0 -= offset;
            }
        }
        for frame in &mut self.frames {
            frame.transform.p -= offset;
            frame.center -= offset;
        }
        for &body in &self.solver.targets {
            if let Some(target) = &mut self.bodies.get_mut(body).target {
                target.p -= offset;
            }
        }
        for geometry in &mut self.geometry {
            for point in geometry.manifold.points_mut() {
                point.point -= offset;
            }
        }
        let executor = self.executor.clone();
        self.broadphase.shift_origin(offset, &*executor);
    }
}
