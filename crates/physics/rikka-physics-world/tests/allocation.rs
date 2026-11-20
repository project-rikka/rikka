use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use rikka_physics_world::{Body, BodyId, Cuboid, Plane, Sphere, Spherical, Transform, Vec3, World};

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}

fn counting() -> bool {
    COUNTING.try_with(Cell::get).unwrap_or(false)
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if counting() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if counting() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

struct Scene {
    world: World,
    head: BodyId,
}

fn scene() -> Scene {
    let mut world = World::builder().build().unwrap();
    world.create_body(Body::fixed().shape(Plane::up())).unwrap();
    world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 0.3, 0.0))
                .shape(Cuboid::new(Vec3::splat(0.08))),
        )
        .unwrap();
    let head = world
        .create_body(
            Body::kinematic()
                .position(Vec3::new(0.0, 1.0, 0.0))
                .shape(Sphere::new(0.1)),
        )
        .unwrap();
    let bob = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 1.0, 0.25))
                .shape(Sphere::new(0.03)),
        )
        .unwrap();
    world
        .create_joint(
            Spherical::new(head, bob).frame(Transform::from_translation(Vec3::new(0.0, 1.0, 0.12))),
        )
        .unwrap();
    Scene { world, head }
}

fn frame(scene: &mut Scene, index: u32) {
    let x = 0.1 * (index as f32 * 0.1).sin();
    scene
        .world
        .move_to(
            scene.head,
            Transform::from_translation(Vec3::new(x, 1.0, 0.0)),
        )
        .unwrap();
    match index % 2 {
        0 => scene.world.advance(1.0 / 60.0),
        _ => {
            let until = scene.world.motion_time() + 1.0 / 60.0;
            scene.world.advance_to(until);
        }
    }
}

#[test]
fn steady_frames_do_not_allocate() {
    let mut scene = scene();
    for index in 0..240 {
        frame(&mut scene, index);
    }
    COUNTING.set(true);
    for index in 240..300 {
        frame(&mut scene, index);
    }
    COUNTING.set(false);
    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0);
}
