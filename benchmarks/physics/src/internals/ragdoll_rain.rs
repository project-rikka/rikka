use rikka_physics_geometry::Cuboid;
use rikka_physics_math::{Transform, Vec3};
use rikka_physics_rigid::{Body, Ragdoll, RagdollBodies, RigidWorld, Spring};

use super::WorkloadResult;
use super::rigid::stepped_scene;

const GRID_COUNT: usize = 10;
const GRID_SIZE: f32 = 15.0;
const GROUP_SIZE: usize = 3;
const DELAY_MASK: u32 = 0x2F;

struct Rain {
    ragdoll: Ragdoll,
    groups: Vec<Vec<RagdollBodies>>,
    columns: usize,
    column: usize,
}

impl Rain {
    fn spawn(
        &mut self,
        world: &mut RigidWorld,
        row: usize,
        column: usize,
    ) -> rikka_physics_rigid::Result<()> {
        let span = GRID_SIZE * GRID_COUNT as f32;
        let mut position = Vec3::new(
            -0.5 * span + GRID_SIZE * (column as f32 + 0.5),
            20.0,
            -0.5 * span + GRID_SIZE * (row as f32 + 0.5),
        );
        let group = &mut self.groups[row * GRID_COUNT + column];
        for _ in 0..GROUP_SIZE {
            group.push(
                self.ragdoll
                    .spawn(world, Transform::from_translation(position))?,
            );
            position.x += 0.75;
        }
        Ok(())
    }

    fn clear(
        &mut self,
        world: &mut RigidWorld,
        row: usize,
        column: usize,
    ) -> rikka_physics_rigid::Result<()> {
        for ragdoll in self.groups[row * GRID_COUNT + column].drain(..) {
            for &body in ragdoll.bodies() {
                world.destroy_body(body)?;
            }
        }
        Ok(())
    }

    fn step(&mut self, world: &mut RigidWorld, step: u32) -> rikka_physics_rigid::Result<()> {
        if step & DELAY_MASK != 0 {
            return Ok(());
        }
        if self.columns < GRID_COUNT {
            for row in 0..GRID_COUNT {
                self.spawn(world, row, self.columns)?;
            }
            self.columns += 1;
        } else {
            for row in 0..GRID_COUNT {
                self.clear(world, row, self.column)?;
                self.spawn(world, row, self.column)?;
            }
            self.column = (self.column + 1) % GRID_COUNT;
        }
        Ok(())
    }
}

pub(crate) fn create(threads: usize) -> WorkloadResult {
    stepped_scene(
        threads,
        true,
        |world| {
            let half = 0.5 * GRID_SIZE * GRID_COUNT as f32;
            world.create_body(
                Body::fixed()
                    .position(Vec3::new(0.0, -1.0, 0.0))
                    .shape(Cuboid::new(Vec3::new(half, 1.0, half))),
            )?;
            Ok(Rain {
                ragdoll: Ragdoll::humanoid()
                    .joint_friction(5.0)
                    .joint_spring(Spring::hertz(1.0).damping_ratio(0.7)),
                groups: (0..GRID_COUNT * GRID_COUNT).map(|_| Vec::new()).collect(),
                columns: 0,
                column: 0,
            })
        },
        |rain, world, step| rain.step(world, step),
    )
}
