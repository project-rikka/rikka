# Third-party notices

## rikka-physics-geometry (convex)

`rikka-physics-geometry/src/convex` ports code from [box3d](https://github.com/erincatto/box3d) at commit `adec25b`:

- `distance.c`: GJK shape distance with simplex cache, segment distance, pairwise shape cast, and time of impact by conservative advancement.
- `hull.c`: Quickhull, hull bulk properties, cylinder, cone and rock hulls, the box hull topology, and transformed hull clones.
- `convex_manifold.c` and `manifold.c`: separating-axis queries with Gauss-map pruning, face clipping, contact reduction, and the sphere, capsule and hull contact routines.
- `sphere.c`, `capsule.c`: ray casts and mass properties.

```
MIT License

Copyright (c) 2026 Erin Catto

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## rikka-physics-geometry (triangulated)

`rikka-physics-geometry/src/triangulated` ports code from [box3d](https://github.com/erincatto/box3d) at commit `adec25b`, under the MIT license above (Copyright (c) 2026 Erin Catto):

- `mesh.c`: binned SAH splitting, triangle ordering, degenerate triangle detection, shared-edge convexity flags, the grid, wave, torus, box, hollow box and platform generators, and the mesh ray, shape cast, overlap and triangle queries.
- `height_field.c`: height quantization, cell triangulation and winding, neighbour convexity flags, hole handling, and the grid and wave generators.
- `compound.c`: child records, shared hull and mesh storage, and the compound queries.
- `triangle_manifold.c`: triangle against sphere, capsule and hull contact with the separating-axis cache.
- `mesh_contact.c`: the triangle cache refresh, ghost-edge removal, normal clustering and contact point culling.
- `simd.c`: the watertight one-sided ray-triangle test.

## rikka-physics-collision

`rikka-physics-collision` ports code from [box3d](https://github.com/erincatto/box3d) at commit `adec25b`, under the MIT license above (Copyright (c) 2026 Erin Catto):

- `physics_world.c`: contact recycling from cached relative poses and anchors, and the collide task's disjoint and touching state changes.
- `broad_phase.c`: the move buffer, pair filtering and deduplicated pair creation.
- `convex_manifold.c`: the sphere, capsule and box contact points used by the wide primitive kernels.
- `contact.c`: manifold feature matching for warm starting and material mixing.

## rikka-physics-rigid

`rikka-physics-rigid` ports code from [box3d](https://github.com/erincatto/box3d) at commit `adec25b`, under the MIT license above (Copyright (c) 2026 Erin Catto):

- `solver.c` and `solver.h`: the Soft Step stage sequence, soft constraint coefficients, velocity integration with the gyroscopic Newton step, position integration and body finalization.
- `contact_solver.c`: wide contact preparation, warm starting, the biased push, relaxation with central friction, twist friction and rolling resistance, restitution and impulse storage.
- `constraint_graph.c`: graph colouring with dynamic constraints from low colours and static ones from high colours, and the overflow colour.
- `body.h`, `solver_set.c`, `island.c`: the 64-byte body state, solver sets, island linking, splitting, sleeping and waking.
- `contact.c`, `joint.c`, `spherical_joint.c`, `weld_joint.c`: contact and joint lifecycles, and the spherical and weld joint solvers.
- `revolute_joint.c`, `prismatic_joint.c`, `wheel_joint.c`, `motor_joint.c`, `parallel_joint.c`, `distance_joint.c`: the hinge, slider, wheel, motor, parallel and distance row formulations (limit rows with speculation, springs, capped motors) behind the shared axis joint and the distance joint.
- `shared/human.c`: the humanoid ragdoll bone hierarchy, joint limits and joint friction factors.

## rikka-physics-collision and rikka-physics-rigid: continuous collision, sensors and movers

These parts of `rikka-physics-collision` and `rikka-physics-rigid` port code from [box3d](https://github.com/erincatto/box3d) at commit `adec25b`, under the MIT license above (Copyright (c) 2026 Erin Catto):

- `solver.c`: continuous collision of fast bodies against static shapes and of bullets against static, kinematic and non-bullet dynamic shapes, with the time-loss gravity correction.
- `shape.c`: shape time of impact, including the mesh and compound triangle callbacks with the parallel-movement early out and the centroid fallback.
- `sensor.c`: sensor overlap gathering and sorted begin and end event diffing.
- `mover.c`: the plane solver and velocity clipping for character movers.
- `shape.c`, `sphere.c`, `capsule.c`, `hull.c`, `mesh.c`, `compound.c`, `physics_world.c`: mover collision planes per shape type and the world mover collide and cast queries.
- `samples/mover.cpp`: the collide, solve and cast iteration of the character mover and pushing dynamic bodies.
