# Third-party code and methods

rikka-physics-fluid is written for Rikka. Some kernels port code or follow published methods closely; their sources are credited here.

## NVIDIA PhysX 5 (BSD-3-Clause)

Copyright (c) 2008-2024 NVIDIA Corporation. All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met: redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer; redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution; neither the name of NVIDIA CORPORATION nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.

Ported from `physx/source/gpusimulationcontroller/src/CUDA/`:

- `matrixDecomposition.cuh` `jacobiRotate` / `eigenDecomposition`: `jacobi_rotate` and `eigen_decompose` in `shaders/lib/fluid/shape.slang`.
- `anisotropy.cu` `smoothPositionsLaunch` / `calculateAnisotropyLaunch`: the weighting `1 - (r/h)^3`, the smoothing blend `min(1, ws / (4 W(h/2)))`, and the clamped square-root eigenvalue radii in `shaders/look_shape.slang`.
- `particlesystem.cu` neighbour layout: the strided per-particle neighbour list (`slot * stride + particle`) in `shaders/neighbours.slang`.
- `PxgParticleSystemCore.cpp` `CalculateRestDensity`: rest density measured on a packed lattice, in `src/lattice.rs`.

## SPlisHSPlasH (MIT)

Copyright (c) 2016-present, SPlisHSPlasH contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software, to deal in the software without restriction, subject to including the copyright notice and permission notice in all copies.

- `SurfaceTension_Akinci2013.cpp`: normals `h Σ V ∇W` and the cohesion spline with the `2ρ0 / (ρi + ρj)` correction, in `shaders/density.slang` and `shaders/solve.slang`.

## Methods

- Macklin & Müller 2013, Position Based Fluids: the density constraint and λ projection (`density.slang`, `solve.slang`).
- Macklin et al. 2016, XPBD: compliant bond constraints with accumulated λ (`solve.slang`).
- Clavet, Beaudoin & Poulin 2005, Particle-based Viscoelastic Fluid Simulation: plastic springs with yield and rest-length flow (`bond_relax.slang`).
- Weiler, Koschier, Brand & Bender 2018, A Physically Consistent Implicit Viscosity Solver for SPH Fluids: the viscosity Laplacian, block-Jacobi preconditioned conjugate gradients, warm start and boundary viscosity (`visc_*.slang`).
- Akinci et al. 2013, Versatile Surface Tension and Adhesion for SPH Fluids: cohesion and boundary adhesion.
- Ihmsen et al. 2012, Unified Spray, Foam and Bubbles for Particle-Based Fluids: trapped-air, wave-crest and kinetic-energy potentials and the spray/foam/bubble classes (`curl.slang`, `foam_*.slang`).
- Yu & Turk 2013, Reconstructing Surfaces of Particle-Based Fluids Using Anisotropic Kernels (`look_shape.slang`).
- Ericson 2005, Real-Time Collision Detection: closest point on a triangle (`closest_on_triangle` in `rikka-physics-gpu/shaders/lib/physics/triangle.slang`).
