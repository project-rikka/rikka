use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rikka_physics_core::{Block, Executor, Stage, WorkerPool};
use rikka_physics_geometry::{Cuboid, Sphere};
use rikka_physics_math::{Quat, Transform, Vec3};
use rikka_physics_rigid::{Body, RigidWorld, ShapeDef};

struct Counting;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static STEPPING: Cell<bool> = const { Cell::new(false) };
}

fn counted() -> bool {
    COUNTING.load(Ordering::Relaxed) && STEPPING.try_with(Cell::get).unwrap_or(false)
}

struct Marking(WorkerPool);

impl Executor for Marking {
    fn workers(&self) -> usize {
        self.0.workers()
    }

    fn run(&self, stages: &[Stage], task: &(dyn Fn(Block) + Sync)) {
        self.0.run(stages, &|block| {
            STEPPING.set(true);
            task(block);
        });
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if counted() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if counted() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn scene(workers: usize) -> (RigidWorld, rikka_physics_rigid::BodyId) {
    let pool = WorkerPool::builder().workers(workers).build().unwrap();
    let executor: Arc<dyn Executor> = Arc::new(Marking(pool));
    let mut world = RigidWorld::builder().workers(executor).build().unwrap();
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(0.0, -1.0, 0.0))
                .shape(Cuboid::new(Vec3::new(20.0, 1.0, 20.0))),
        )
        .unwrap();
    let platform = world
        .create_body(
            Body::kinematic()
                .position(Vec3::new(0.0, 0.5, 0.0))
                .shape(Cuboid::new(Vec3::new(2.0, 0.25, 2.0))),
        )
        .unwrap();
    for i in 0..12 {
        world
            .create_body(
                Body::dynamic()
                    .position(Vec3::new(-1.5 + (i % 4) as f32, 1.5 + (i / 4) as f32, 0.0))
                    .shape(Sphere::new(0.3)),
            )
            .unwrap();
    }
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(4.0, 1.0, 0.0))
                .shape(ShapeDef::new(Cuboid::new(Vec3::splat(1.0))).sensor(true)),
        )
        .unwrap();
    (world, platform)
}

#[test]
fn steady_steps_do_not_allocate() {
    for workers in [1, 4] {
        let (mut world, platform) = scene(workers);
        let drive = |world: &mut RigidWorld, step: u32| {
            let angle = step as f32 * 0.02;
            let target = Transform::new(
                Vec3::new(angle.sin(), 0.5, 0.0),
                Quat::from_rotation_y(0.5 * angle),
            );
            world.move_to(platform, target).unwrap();
            world.step(1.0 / 60.0);
        };
        for step in 0..240 {
            drive(&mut world, step);
        }
        ALLOCATIONS.store(0, Ordering::Relaxed);
        STEPPING.set(true);
        COUNTING.store(true, Ordering::Relaxed);
        for step in 240..360 {
            drive(&mut world, step);
        }
        COUNTING.store(false, Ordering::Relaxed);
        assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0, "{workers} workers");
    }
}
