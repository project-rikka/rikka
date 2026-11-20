import cloth.solve.positions;

float limb_depth(Collider collider, ReadVec4s planes, float3 p, float3 q, out float3 deepest) {
    float most = -FOREVER;
    deepest = p;
    for (uint k = 1u; k <= LIMB_SAMPLES; ++k) {
        float3 x = lerp(p, q, float(k) / float(LIMB_SAMPLES + 1u));
        float3 outward;
        float2 found = collider_distance(collider, planes, x, outward);
        float depth = -found.x - LIMB_CORE * found.y;
        if (depth > most) {
            most = depth;
            deepest = x;
        }
    }
    return most;
}

bool limb_advanced(Collider now, Collider then, float3 x) {
    float3 outward;
    return collider_distance(now, job.planes_to, x, outward).x < collider_distance(then, job.planes_from, x, outward).x;
}

float3 limb_sided(Collider now, Collider then, float3 x, float3 origin, float radius, float3 side) {
    float3 outward;
    collider_distance(now, job.planes_to, x, outward);
    if (dot(outward, side) >= 0.0) {
        return float3(0.0);
    }
    return collider_exit(now, then, job.planes_to, job.planes_from, x, origin, radius);
}

void keep_limb_side(uint i, uint j, float wi, float wj) {
    float ri = job.traits[i].b.w;
    float rj = job.traits[j].b.w;
    if (ri < 0.0 || rj < 0.0 || (wi <= 0.0 && wj <= 0.0)) {
        return;
    }
    uint near = job.contacts[i * CONTACT_WORDS + FLAG_WORD] | job.contacts[j * CONTACT_WORDS + FLAG_WORD];
    if (near == NONE || (near & NEAR_LIMB) == 0u) {
        return;
    }
    uint packed_i = job.contacts[i * CONTACT_WORDS + COLLIDER_WORD];
    uint packed_j = job.contacts[j * CONTACT_WORDS + COLLIDER_WORD];
    uint indices[2] = { packed_i & 0xffffu, packed_j & 0xffffu };
    float3 origin = 0.5 * (job.epoch_start[i].xyz + job.epoch_start[j].xyz);
    float3 xi = position(i);
    float3 xj = position(j);
    bool moved = false;
    for (uint k = 0u; k < 2u; ++k) {
        uint index = indices[k];
        if (index == NO_COLLIDER || (k == 1u && index == indices[0])) {
            continue;
        }
        Collider now = job.colliders_to[index];
        if (now.shape.x == 0u) {
            float3 outward;
            float2 at_i = collider_distance(now, job.planes_to, xi, outward);
            float2 at_j = collider_distance(now, job.planes_to, xj, outward);
            if (0.5 * (at_i.x + at_j.x - distance(xi, xj)) > -LIMB_CORE * at_i.y) {
                continue;
            }
        } else if (hull_pair_past(now, job.planes_to, xi, xj, distance(xi, xj) - 2.0 * LIMB_CORE * asfloat(now.shape.w))) {
            continue;
        }
        float3 deepest;
        if (limb_depth(now, job.planes_to, xi, xj, deepest) <= 0.0) {
            continue;
        }
        Collider then = job.colliders_from[index];
        float3 was;
        if (limb_depth(then, job.planes_from, job.epoch_start[i].xyz, job.epoch_start[j].xyz, was) > 0.0
            || !limb_advanced(now, then, deepest)) {
            continue;
        }
        float3 exit = collider_exit(now, then, job.planes_to, job.planes_from, deepest, origin, 0.0);
        float out_by = length(exit);
        if (!(out_by > 1.0e-9)) {
            continue;
        }
        float3 side = exit / out_by;
        float3 shift_i = wi > 0.0 ? limb_sided(now, then, xi, origin, ri, side) : float3(0.0);
        float3 shift_j = wj > 0.0 ? limb_sided(now, then, xj, origin, rj, side) : float3(0.0);
        float span = distance(xi, xj);
        if (length(shift_i) > 0.85 * span || length(shift_j) > 0.85 * span) {
            continue;
        }
        xi += shift_i;
        xj += shift_j;
        if (limb_depth(now, job.planes_to, xi, xj, deepest) > 0.0) {
            float3 lift = clamp_length(
                collider_exit(now, then, job.planes_to, job.planes_from, deepest, origin, 0.0),
                0.5 * span);
            xi += wi > 0.0 ? lift : float3(0.0);
            xj += wj > 0.0 ? lift : float3(0.0);
        }
        moved = true;
        if (wi > 0.0) {
            halt(i, side);
        }
        if (wj > 0.0) {
            halt(j, side);
        }
    }
    if (moved) {
        if (wi > 0.0) {
            place(i, xi);
        }
        if (wj > 0.0) {
            place(j, xj);
        }
    }
}

void retidy_edges(bool damp_still, bool strict_outward) {
    float still_damping = damp_still ? job.rest_terms.x : 0.0;
    float axial_damp = damp_still ? per_substep(AXIAL_DAMP, job.step.x) : 0.0;
    for (uint colour = 0u; colour < job.colours.y; ++colour) {
        uint from = plan(job.colours.x + 2u * colour);
        uint to = plan(job.colours.x + 2u * colour + 1u);
        for (uint e = from + lane; e < to; e += THREADS) {
            Edge edge = job.edges[e];
            float3 xi = position(edge.ends.x);
            float3 xj = position(edge.ends.y);
            float3 d = xj - xi;
            float len = length(d);
            float longest = asfloat(edge.ends.w);
            float wi = edge.params.z;
            float wj = edge.params.w;
            float pressed_i = job.starts[edge.ends.x].w;
            float pressed_j = job.starts[edge.ends.y].w;
            float touch_i = pressed_touch(pressed_i);
            float touch_j = pressed_touch(pressed_j);
            float free_i = edge.params.z;
            float free_j = edge.params.w;
            bool tearing = len > TEARING * longest;
            bool hard_tearing = !strict_outward && len > HARD_TEARING * longest;
            if (free_j > 0.0 && !tearing) {
                wi *= 1.0 - TOUCH_LOCK * touch_i * (1.0 - touch_j);
            }
            if (free_i > 0.0 && !tearing) {
                wj *= 1.0 - TOUCH_LOCK * touch_j * (1.0 - touch_i);
            }
            float weights = wi + wj;
            if (len > longest && weights > 0.0) {
                float3 n = d * ((len - longest) / (weights * len));
                float3 toward = d / len;
                if (wi > 0.0) {
                    place(edge.ends.x, xi + (hard_tearing ? n * wi : clamp_outward(n * wi, pressed_i)));
                    halt(edge.ends.x, toward);
                }
                if (wj > 0.0) {
                    place(edge.ends.y, xj + (hard_tearing ? -n * wj : clamp_outward(-n * wj, pressed_j)));
                    halt(edge.ends.y, -toward);
                }
            }
            if (damp_still) {
                bool both_free = edge.params.z > 0.0 && edge.params.w > 0.0;
                float axial = both_free ? axial_damp : 0.0;
                float inertia = edge.params.z + edge.params.w;
                if (inertia > 0.0 && (still_damping > 0.0 || (axial > 0.0 && len > 1.0e-12))) {
                    float4 vi = job.velocities[edge.ends.x];
                    float4 vj = job.velocities[edge.ends.y];
                    float3 diff = vj.xyz - vi.xyz;
                    float3 relative = diff * (still_damping / inertia);
                    if (axial > still_damping && len > 1.0e-12) {
                        float3 dir = d / len;
                        relative += dir * (dot(diff, dir) * ((axial - still_damping) / inertia));
                    }
                    job.velocities[edge.ends.x] = float4(vi.xyz + clamp_outward(relative * edge.params.z, pressed_i), vi.w);
                    job.velocities[edge.ends.y] = float4(vj.xyz + clamp_outward(-relative * edge.params.w, pressed_j), vj.w);
                }
            }
        }
        sync();
    }
}

void retidy(bool sided) {
    if (sided && job.shapes_figure.w > 0u) {
        for (uint colour = 0u; colour < job.colours.y; ++colour) {
            uint from = plan(job.colours.x + 2u * colour);
            uint to = plan(job.colours.x + 2u * colour + 1u);
            for (uint e = from + lane; e < to; e += THREADS) {
                Edge edge = job.edges[e];
                keep_limb_side(edge.ends.x, edge.ends.y, edge.params.z, edge.params.w);
            }
            sync();
        }
    }
    retidy_edges(true, false);
    if (!sided) {
        retidy_edges(false, false);
    }
}
