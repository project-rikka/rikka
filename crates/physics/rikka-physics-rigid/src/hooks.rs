use rikka_physics_collision::ContactManifold;

use crate::ShapeId;

pub trait PreSolve: Send + Sync {
    fn pre_solve(&self, shape_a: ShapeId, shape_b: ShapeId, manifold: &ContactManifold) -> bool;
}

impl<F: Fn(ShapeId, ShapeId, &ContactManifold) -> bool + Send + Sync> PreSolve for F {
    fn pre_solve(&self, shape_a: ShapeId, shape_b: ShapeId, manifold: &ContactManifold) -> bool {
        self(shape_a, shape_b, manifold)
    }
}

pub trait CollisionFilter: Send + Sync {
    fn should_collide(&self, shape_a: ShapeId, shape_b: ShapeId) -> bool;
}

impl<F: Fn(ShapeId, ShapeId) -> bool + Send + Sync> CollisionFilter for F {
    fn should_collide(&self, shape_a: ShapeId, shape_b: ShapeId) -> bool {
        self(shape_a, shape_b)
    }
}
