import cloth.solve.positions;

void stretch(float h) {
    float inverse_h2 = 1.0 / (h * h);
    for (uint colour = job.colours.y; colour < job.colours.y + job.colours.z; ++colour) {
        uint from = plan(job.colours.x + 2u * colour);
        uint to = plan(job.colours.x + 2u * colour + 1u);
        for (uint e = from + lane; e < to; e += THREADS) {
            Edge edge = job.edges[e];
            float3 xi = position(edge.ends.x);
            float3 xj = position(edge.ends.y);
            float3 d = xj - xi;
            float len = length(d);
            float rest = asfloat(edge.ends.z);
            float c = len - rest;
            float alpha = c > 0.0 ? edge.params.x : edge.params.y;
            float weights = edge.params.z + edge.params.w;
            if (isinf(alpha) || !(len > 1.0e-12) || weights == 0.0) {
                continue;
            }
            float step = c / (weights + alpha * inverse_h2);
            float3 n = d / len;
            if (edge.params.z > 0.0) {
                place(edge.ends.x, xi + n * (edge.params.z * step));
            }
            if (edge.params.w > 0.0) {
                place(edge.ends.y, xj - n * (edge.params.w * step));
            }
        }
        sync();
    }
}

float unwind(float angle) {
    if (angle > PI) {
        return angle - 2.0 * PI;
    }
    if (angle < -PI) {
        return angle + 2.0 * PI;
    }
    return angle;
}

void bend_slots(uint item, float h, float inverse_h2) {
    Hinge bend = job.bends[item];
    float3 x0 = position(bend.ids.x);
    float3 x1 = position(bend.ids.y);
    float3 x2 = position(bend.ids.z);
    float3 x3 = position(bend.ids.w);
    float3 e = x1 - x0;
    float l2 = dot(e, e);
    float3 big1 = cross(e, x2 - x0);
    float3 big2 = cross(x3 - x0, e);
    float q1 = dot(big1, big1);
    float q2 = dot(big2, big2);
    float lambda = 0.0;
    float3 g0 = float3(0.0);
    float3 g1 = float3(0.0);
    float3 g2 = float3(0.0);
    float3 g3 = float3(0.0);
    float cap = 0.0;
    if (l2 > 1.0e-12 && q1 > 1.0e-8 * l2 * l2 && q2 > 1.0e-8 * l2 * l2) {
        float l = sqrt(l2);
        cap = 0.25 * l;
        float3 n1 = big1 * rsqrt(q1);
        float3 n2 = big2 * rsqrt(q2);
        float theta = atan2(dot(cross(n1, n2), e / l), dot(n1, n2));
        float c = unwind(theta - bend.params.x);
        float buckled = smoothstep(0.5 * bend.params.w, 1.5 * bend.params.w, abs(c));
        float alpha = 1.0 / lerp(1.0 / bend.params.y, 1.0 / bend.params.z, buckled);
        g2 = big1 * (-l / q1);
        g3 = big2 * (-l / q2);
        float s2 = dot(x2 - x0, e) / l2;
        float s3 = dot(x3 - x0, e) / l2;
        g1 = -(g2 * s2 + g3 * s3);
        g0 = -(g2 * (1.0 - s2) + g3 * (1.0 - s3));
        float4 w = bend.weights;
        float total = w.x * dot(g0, g0) + w.y * dot(g1, g1) + w.z * dot(g2, g2) + w.w * dot(g3, g3);
        if (!isinf(alpha) && total > 1.0e-20) {
            float rate_c = (w.x > 0.0 ? dot(g0, x0 - job.starts[bend.ids.x].xyz) : 0.0)
                + (w.y > 0.0 ? dot(g1, x1 - job.starts[bend.ids.y].xyz) : 0.0)
                + (w.z > 0.0 ? dot(g2, x2 - job.starts[bend.ids.z].xyz) : 0.0)
                + (w.w > 0.0 ? dot(g3, x3 - job.starts[bend.ids.w].xyz) : 0.0);
            float gamma = BEND_RAYLEIGH / h;
            lambda = -(c + gamma * rate_c) / ((1.0 + gamma) * total + alpha * inverse_h2);
        }
    }
    float4 applied = bend.applied * lambda;
    float3 d0 = g0 * applied.x;
    float3 d1 = g1 * applied.y;
    float3 d2 = g2 * applied.z;
    float3 d3 = g3 * applied.w;
    if (cap > 0.0) {
        d0 = clamp_length(d0, cap);
        d1 = clamp_length(d1, cap);
        d2 = clamp_length(d2, cap);
        d3 = clamp_length(d3, cap);
    }
    slot(bend.dests.x, d0);
    slot(bend.dests.y, d1);
    slot(bend.dests.z, d2);
    slot(bend.dests.w, d3);
}
float3 swing_toward(float3 aim, float3 direction, float limit) {
    float3 axis = cross(aim, direction);
    float axis_squared = dot(axis, axis);
    if (!(axis_squared > 1.0e-12)) {
        float3 other = abs(aim.x) < 0.9 ? float3(1.0, 0.0, 0.0) : float3(0.0, 1.0, 0.0);
        axis = cross(aim, other);
        axis_squared = dot(axis, axis);
    }
    return rotate_about(aim, axis * rsqrt(axis_squared), limit);
}

void shape_slots(uint item, float alpha, float h) {
    Hinge shape = job.shapes[item];
    uint child = shape.ids.x;
    uint parent = shape.ids.y;
    float3 xc = position(child);
    float3 xp = position(parent);
    float loose = clamp(job.pieces[job.traits[child].d.x].grab.x, 0.0, 1.0);
    float slack = (1.0 - clamp(job.velocities[child].w, 0.0, 1.0)) * (1.0 - loose);
    float3 dc = float3(0.0);
    float3 dp = float3(0.0);
    float3 dg = float3(0.0);
    float share = per_substep(shape.params.w, h) * slack;
    float total = shape.weights.x + shape.weights.y;
    if (share > 0.0 && total > 0.0) {
        float3 apart = xc - xp - (target(child, alpha) - target(parent, alpha));
        float gain = RESTORE_GAIN * share / (1.0 + (RESTORE_GAIN - 1.0) * share);
        float3 lambda = apart * (gain / total);
        dc -= lambda * shape.applied.x;
        dp += lambda * shape.applied.y;
    }
    float limit = shape.params.y;
    float stiff = per_substep(shape.params.z, h) * slack;
    float3 outgoing = xc - xp;
    float out_len = length(outgoing);
    if (limit >= 0.0 && stiff > 0.0 && out_len > 1.0e-9) {
        float3 a_dir = outgoing / out_len;
        if (shape.ids.z == NONE) {
            float3 posed = target(child, alpha) - target(parent, alpha);
            float3 aim = posed * rsqrt(max(dot(posed, posed), 1.0e-24));
            if (dot(a_dir, aim) < cos(limit) && shape.weights.x > 0.0) {
                float3 limited = xp + swing_toward(aim, a_dir, limit) * out_len;
                dc += (limited - xc) * stiff * shape.applied.x / shape.weights.x;
            }
        } else {
            uint grandparent = shape.ids.z;
            float3 incoming = xp - position(grandparent);
            float in_len = length(incoming);
            if (in_len > 1.0e-9) {
                float3 b_dir = incoming / in_len;
                float cosine = clamp(dot(a_dir, b_dir), -1.0, 1.0);
                float theta = acos(cosine);
                float rest = shape.params.x;
                float c = theta > rest + limit ? theta - rest - limit : (theta < rest - limit ? theta - rest + limit : 0.0);
                float sine = sqrt(max(1.0 - cosine * cosine, 0.0));
                if (c != 0.0 && sine > 0.02) {
                    float3 gc = -(b_dir - cosine * a_dir) / (out_len * sine);
                    float3 gg = (a_dir - cosine * b_dir) / (in_len * sine);
                    float3 gp = -(gc + gg);
                    float4 w = shape.weights;
                    float total = w.x * dot(gc, gc) + w.y * dot(gp, gp) + w.z * dot(gg, gg);
                    if (total > 1.0e-20) {
                        float lambda = -c * stiff / total;
                        dc += gc * (shape.applied.x * lambda);
                        dp += gp * (shape.applied.y * lambda);
                        dg += gg * (shape.applied.z * lambda);
                    }
                }
            }
        }
    }
    float cap = 0.25 * max(out_len, 0.005);
    slot(shape.dests.x, clamp_length(dc, cap));
    slot(shape.dests.y, clamp_length(dp, cap));
    slot(shape.dests.z, clamp_length(dg, cap));
}

void wrap_slots(uint t, float alpha) {
    if (job.wraps[t * WRAP_CANDIDATES] == NONE) {
        return;
    }
    uint4 corners = job.triangles[t];
    uint4 dests = job.wrap_dests[t];
    float3 a = position(corners.x);
    float3 b = position(corners.y);
    float3 c = position(corners.z);
    float3 n = face_normal(a, b, c);
    float3 da = float3(0.0);
    float3 db = float3(0.0);
    float3 dc = float3(0.0);
    if (any(n != float3(0.0))) {
        float thick = WRAP_SHARE * min(min(job.traits[corners.x].b.w, job.traits[corners.y].b.w), job.traits[corners.z].b.w);
        float3 weights = float3(job.traits[corners.x].a.x, job.traits[corners.y].a.x, job.traits[corners.z].a.x);
        float3 movable = float3(weights > float3(0.0));
        float total = 0.0;
        for (uint k = 0u; k < WRAP_CANDIDATES; ++k) {
            uint entry = job.wraps[t * WRAP_CANDIDATES + k];
            if (entry == NONE || thick < 0.0) {
                break;
            }
            float3 v = skinned_body(job, entry & INDEX_MASK, alpha);
            float side = (entry & ABOVE) != 0u ? 1.0 : -1.0;
            float d = dot(v - a, n);
            float3 bary = closest_on_triangle(v - n * d, a, b, c).weights;
            float inner = clamp(min(min(bary.x, bary.y), bary.z) / WRAP_RIM, 0.0, 1.0);
            float share = clamp(2.0 * dot(movable, bary) - 0.5, 0.0, 1.0) * inner;
            float amount = min(thick - side * d, PUSH_SPEED * job.step.x) * share;
            if (!(amount > 0.0)) {
                continue;
            }
            float3 direction = -side * n;
            float3 push = direction * (amount * amount / dot(weights, bary * bary));
            da += push * (weights.x * bary.x);
            db += push * (weights.y * bary.y);
            dc += push * (weights.z * bary.z);
            total += amount;
        }
        if (total > 0.0) {
            da /= total;
            db /= total;
            dc /= total;
        }
    }
    slot(dests.x, da * asfloat(job.traits[corners.x].d.z));
    slot(dests.y, db * asfloat(job.traits[corners.y].d.z));
    slot(dests.z, dc * asfloat(job.traits[corners.z].d.z));
}

void self_contact(uint i, float alpha) {
    uint base = i * CONTACT_WORDS;
    float3 push = float3(0.0);
    float pushes = 0.0;
    float strongest = 0.0;
    uint partner = NONE;
    Trait trait = job.traits[i];
    if (trait.a.x != 0.0 && (job.contacts[base + FLAG_WORD] & TANGLED) == 0u) {
        float3 x = position(i);
        float thick = trait.b.z;
        float closest_gap = FOREVER;
        uint inner = job.pieces[trait.d.x].layer_triangles.x;
        for (uint k = 0u; k < CLOTH_CANDIDATES; ++k) {
            uint entry = job.contacts[base + CLOTH_WORD + k];
            if (entry == NONE) {
                break;
            }
            bool own = obstacle_figure(entry) == job.shapes_figure.z;
            if (inner_sheet(entry, inner)) {
                continue;
            }
            float3 a;
            float3 b;
            float3 c;
            uint4 corners;
            cloth_triangle(entry, alpha, a, b, c, corners);
            Gap g = gap_to(x, a, b, c, entry);
            float share = own && mine(corners.x) ? 0.5 : 1.0;
            float cover = drawn_cover(entry, corners, g, a, b, c);
            float apart = g.valid ? g.gap - cover : FOREVER;
            float needed = own && obstacle_index(entry) < inner ? snug_thickness(i, corners, thick) : thick;
            if (apart < needed) {
                float amount = (needed - apart) * share;
                push += g.normal * amount;
                pushes += amount;
                strongest = max(strongest, amount);
                if (apart < closest_gap) {
                    closest_gap = apart;
                    partner = k;
                }
            }
            if (k > 0u || !g.valid || g.gap > 2.0 * thick) {
                continue;
            }
            ReadVec4s other_start = own ? job.epoch_start : job.figures[obstacle_figure(entry)].epoch_start;
            uint to = job.incident_starts[i + 1u];
            for (uint at = job.incident_starts[i]; at < to; ++at) {
                uint4 near = job.triangles[job.incident[at]];
                uint j = near.x == i ? near.y : (near.y == i ? near.z : near.x);
                {
                    float3 xj = position(j);
                    float3 points[3] = { a, b, c };
                    uint ids[3] = { corners.x, corners.y, corners.z };
                    for (uint s = 0u; s < 3u; ++s) {
                        uint p = ids[s];
                        uint q = ids[(s + 1u) % 3u];
                        if (own && (p == i || q == i || p == j || q == j)) {
                            continue;
                        }
                        float3 xp = points[s];
                        float3 xq = points[(s + 1u) % 3u];
                        float2 st = segment_params(x, xj, xp, xq);
                        if (st.x <= 0.0 || st.x >= 1.0 || st.y <= 0.0 || st.y >= 1.0) {
                            continue;
                        }
                        float3 on_mine = lerp(x, xj, st.x);
                        float3 on_other = lerp(xp, xq, st.y);
                        float3 apart = on_mine - on_other;
                        float dist = length(apart);
                        if (dist >= thick) {
                            continue;
                        }
                        float3 was = lerp(job.epoch_start[i].xyz, job.epoch_start[j].xyz, st.x)
                            - lerp(other_start[p].xyz, other_start[q].xyz, st.y);
                        float was_len = length(was);
                        if (!(was_len > 1.0e-9)) {
                            continue;
                        }
                        float3 normal = was / was_len;
                        float along = dot(apart, normal);
                        float depth = thick - along;
                        if (depth <= 0.0) {
                            continue;
                        }
                        float amount = depth * (1.0 - st.x) * share;
                        push += normal * amount;
                        pushes += amount;
                        strongest = max(strongest, amount);
                    }
                }
            }
        }
    }
    job.selfs[i] = float4(pushes > 0.0 ? push * (strongest / pushes) : float3(0.0), asfloat(partner));
}

void jacobi(float alpha, float h) {
    float inverse_h2 = 1.0 / (h * h);
    uint bends = job.items_range.y;
    uint shapes = job.shapes_figure.y;
    uint triangles = job.range.w;
    uint total = bends + shapes + triangles + count;
    for (uint k = lane; k < total; k += THREADS) {
        uint at = k;
        if (at < bends) {
            bend_slots(job.items_range.x + at, h, inverse_h2);
            continue;
        }
        at -= bends;
        if (at < shapes) {
            shape_slots(job.shapes_figure.x + at, alpha, h);
            continue;
        }
        at -= shapes;
        if (at < triangles) {
            wrap_slots(job.range.z + at, alpha);
            continue;
        }
        at -= triangles;
        self_contact(first + at, alpha);
    }
}
