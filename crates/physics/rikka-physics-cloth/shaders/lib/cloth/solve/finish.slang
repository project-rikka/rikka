import cloth.solve.positions;

void finish(float h) {
    float rate = 1.0 / h;
    float epoch_rate = 1.0 / (float(job.steps.y) * h);
    for (uint i = first + lane; i < first + count; i += THREADS) {
        Trait trait = job.traits[i];
        float3 x = position(i);
        float3 start = job.starts[i].xyz;
        float4 pred = job.preds[i];
        float3 v;
        if (trait.a.x == 0.0) {
            v = (x - start) * rate;
        } else {
            Piece piece = job.pieces[trait.d.x];
            float3 corrected = x - job.selfs[i].xyz - pred.xyz;
            v = (x - start - piece.response.x * corrected) * rate;
            float pressed = job.starts[i].w;
            if (pressed > 0.0) {
                float3 normal = pressed_normal(pressed);
                v -= normal * (min(dot(v, normal), 0.0) * pressed_touch(pressed));
            }
            uint partner = asuint(job.selfs[i].w);
            if (partner != NONE) {
                uint entry = job.contacts[i * CONTACT_WORDS + CLOTH_WORD + partner];
                if (obstacle_figure(entry) == job.shapes_figure.z) {
                    uint idx = obstacle_index(entry);
                    uint4 corners = job.triangles[idx];
                    if (mine(corners.x) && mine(corners.y) && mine(corners.z)) {
                        if (piece.contact.y > 0.0) {
                            float3 other = ((position(corners.x) - job.starts[corners.x].xyz)
                                + (position(corners.y) - job.starts[corners.y].xyz)
                                + (position(corners.z) - job.starts[corners.z].xyz)) * (rate / 3.0);
                            v += (other - v) * clamp(piece.contact.y, 0.0, 1.0);
                        }
                    } else if (idx < piece.layer_triangles.x) {
                        float3 ta = job.positions[corners.x].xyz;
                        float3 tb = job.positions[corners.y].xyz;
                        float3 tc = job.positions[corners.z].xyz;
                        float3 other = ((ta - job.epoch_start[corners.x].xyz)
                            + (tb - job.epoch_start[corners.y].xyz)
                            + (tc - job.epoch_start[corners.z].xyz)) * (epoch_rate / 3.0);
                        float3 n = face_normal(ta, tb, tc);
                        if (any(n != float3(0.0))) {
                            float side = remembered_side(entry, dot(x - ta, n));
                            float3 outward = side * n;
                            float inward = dot(v - other, outward);
                            if (inward < 0.0) {
                                v -= outward * inward;
                            }
                        }
                        if (piece.contact.y > 0.0) {
                            v += (other - v) * clamp(piece.contact.y, 0.0, 1.0);
                        }
                    }
                }
            }
            float speed = length(v);
            if (speed > SHOCK_SPEED.x) {
                float3 prev_v = job.velocities[i].xyz;
                float prev_speed = length(prev_v);
                if (prev_speed > SHOCK_SPEED.x) {
                    float oppose = -dot(v, prev_v) / (speed * prev_speed);
                    if (oppose > SHOCK_OPPOSE.x) {
                        float shock = smoothstep(SHOCK_OPPOSE.x, SHOCK_OPPOSE.y, oppose)
                            * smoothstep(SHOCK_SPEED.x, SHOCK_SPEED.y, min(speed, prev_speed));
                        float scale = 1.0 - SHOCK_DAMP * shock;
                        v *= scale;
                        speed *= scale;
                    }
                }
            }
            if (speed > piece.response.y) {
                v *= piece.response.y / speed;
            }
        }
        job.velocities[i] = float4(v, clamp(pred.w, 0.0, 1.0));
    }
}

float steadied(uint i, inout float3 x) {
    float3 start = job.epoch_start[i].xyz;
    float3 moved = x - start;
    float4 drift = job.drift[i];
    float3 heading = lerp(moved, drift.xyz, DRIFT_MEMORY);
    float pace = lerp(length(moved), drift.w, DRIFT_MEMORY);
    job.drift[i] = float4(heading, pace);
    float wavering = pace > 1.0e-9 ? smoothstep(TREMBLING.x, TREMBLING.y, 1.0 - length(heading) / pace) : 0.0;
    float freeze = max(job.rest_terms.z * wavering, job.rest_terms.w);
    if (freeze > 0.0) {
        x = lerp(x, start, freeze);
        place(i, x);
    }
    float micro = wavering * (1.0 - smoothstep(MICRO_PACE.x, MICRO_PACE.y, pace));
    float v_damp = max(freeze, MICRO_DAMP * micro);
    if (v_damp > 0.0) {
        float4 v = job.velocities[i];
        job.velocities[i] = float4(v.xyz * (1.0 - v_damp), v.w);
    }
    return wavering;
}
