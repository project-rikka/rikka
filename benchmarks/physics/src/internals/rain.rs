use rikka_physics_collision::Filter;
use rikka_physics_geometry::{MeshShape, TriangleMesh};
use rikka_physics_math::{Transform, Vec3};
use rikka_physics_rigid::{Body, Ragdoll, RagdollBodies, RigidWorld, Spring};

use super::WorkloadResult;
use super::rigid::stepped_scene;

const GRID_SIZE: f32 = 15.0;
const GRID_COUNT: usize = 10;
const GROUP_SIZE: usize = 3;
const DELAY: u32 = 0x2F;

struct Rain {
    ragdoll: Ragdoll,
    groups: Vec<Vec<RagdollBodies>>,
    columns: usize,
    column: usize,
}

impl Rain {
    fn create_group(
        &mut self,
        world: &mut RigidWorld,
        row: usize,
        column: usize,
    ) -> rikka_physics_rigid::Result<()> {
        let index = row * GRID_COUNT + column;
        let span = GRID_SIZE * GRID_COUNT as f32;
        let mut position = Vec3::new(
            -0.5 * span + GRID_SIZE * (column as f32 + 0.5),
            20.0,
            -0.5 * span + GRID_SIZE * (row as f32 + 0.5),
        );
        let ragdoll = self
            .ragdoll
            .filter(Filter::default().group(-(index as i32)));
        for _ in 0..GROUP_SIZE {
            let bodies = ragdoll.spawn(world, Transform::from_translation(position))?;
            self.groups[index].push(bodies);
            position.x += 0.75;
        }
        Ok(())
    }

    fn destroy_group(
        &mut self,
        world: &mut RigidWorld,
        row: usize,
        column: usize,
    ) -> rikka_physics_rigid::Result<()> {
        for bodies in self.groups[row * GRID_COUNT + column].drain(..) {
            for &body in bodies.bodies() {
                world.destroy_body(body)?;
            }
        }
        Ok(())
    }

    fn step(&mut self, world: &mut RigidWorld, step: u32) -> rikka_physics_rigid::Result<()> {
        if step & DELAY != 0 {
            return Ok(());
        }
        if self.columns < GRID_COUNT {
            for row in 0..GRID_COUNT {
                self.create_group(world, row, self.columns)?;
            }
            self.columns += 1;
        } else {
            for row in 0..GRID_COUNT {
                self.destroy_group(world, row, self.column)?;
                self.create_group(world, row, self.column)?;
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
            let half_rows = 4;
            let grid = MeshShape::new(TriangleMesh::grid(
                2 * half_rows,
                2 * half_rows,
                GRID_SIZE / (2 * half_rows) as f32,
                1,
            )?);
            let torus = MeshShape::new(TriangleMesh::torus(16, 16, 0.25 * GRID_SIZE, 1.0)?);
            let span = GRID_SIZE * GRID_COUNT as f32;
            for i in 0..GRID_COUNT {
                for j in 0..GRID_COUNT {
                    let position = Vec3::new(
                        -0.5 * span + GRID_SIZE * (i as f32 + 0.5),
                        0.0,
                        -0.5 * span + GRID_SIZE * (j as f32 + 0.5),
                    );
                    world.create_body(
                        Body::fixed()
                            .position(position)
                            .shape(grid.clone())
                            .shape(torus.clone()),
                    )?;
                }
            }
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
