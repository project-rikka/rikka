import cloth.solve.positions;

float3 gathered(uint i, out float3 wrapped) {
    float3 sum = float3(0.0);
    wrapped = float3(0.0);
    uint to = job.gather_starts[i + 1u];
    uint wraps = to - (job.incident_starts[i + 1u] - job.incident_starts[i]);
    for (uint at = job.gather_starts[i]; at < wraps; ++at) {
        sum += job.slots[at].xyz;
    }
    for (uint at = wraps; at < to; ++at) {
        wrapped += job.slots[at].xyz;
    }
    return sum;
}

float3 tethered(uint i, float3 x, float alpha) {
    uint from = job.tether_starts[i];
    uint to = job.tether_starts[i + 1u];
    float3 pull = float3(0.0);
    float pulling = 0.0;
    for (uint at = from; at < to; ++at) {
        Tether tether = job.tethers[at];
        float3 d = x - target(tether.anchor, alpha);
        float dist = length(d);
        if (dist > tether.longest) {
            pull -= d * ((dist - tether.longest) / dist);
            pulling += 1.0;
        }
    }
    return pulling > 0.0 ? x + pull / pulling : x;
}

float3 extended(uint i, float3 x, float alpha) {
    uint from = job.tether_starts[i];
    uint to = job.tether_starts[i + 1u];
    float3 push = float3(0.0);
    float pushing = 0.0;
    for (uint at = from; at < to; ++at) {
        Tether tether = job.tethers[at];
        float3 d = x - target(tether.anchor, alpha);
        float dist = length(d);
        if (dist < tether.shortest && dist > 1.0e-9) {
            push += d * ((tether.shortest - dist) / dist);
            pushing += 1.0;
        }
    }
    if (pushing == 0.0) {
        return float3(0.0);
    }
    float activity = job.rest_terms.y;
    push *= EXTENT_SHARE * max(activity, EXTENT_HELD) / pushing;
    float pressed = job.starts[i].w;
    if (pressed > 0.0) {
        float3 normal = pressed_normal(pressed);
        push -= normal * min(dot(push, normal), 0.0);
    }
    return push;
}

float3 limited(Trait trait, uint i, float3 x, float alpha, float h, float slack, float loose, out float3 barrier) {
    barrier = float3(0.0);
    float3 posed = target(i, alpha);
    x = lerp(x, posed, per_substep(trait.a.z, h) * slack * (1.0 - loose));
    if (trait.a.w >= 0.0 && loose < 1.0) {
        float reach = trait.a.w / (1.0 - loose);
        float3 d = x - posed;
        float dist = length(d);
        if (dist > reach) {
            x = posed + d * (lerp(dist, reach, REACH_SHARE) / dist);
        }
    }
    if (trait.b.y >= 0.0 && loose < 1.0) {
        float3 normal = target_normal(i, alpha);
        float depth = dot(x - posed, normal);
        if (depth < -trait.b.y) {
            float3 push = normal * (-trait.b.y - depth);
            barrier += push;
            x += push;
        }
    }
    if (job.flags.x != 0u) {
        float floor_y = job.step.z + max(trait.b.z, trait.b.w);
        if (x.y < floor_y) {
            barrier.y += floor_y - x.y;
            x.y = floor_y;
        }
    }
    return x;
}

float3 truncated(uint i, float3 x, float alpha) {
    uint base = i * CONTACT_WORDS;
    if ((job.contacts[base + FLAG_WORD] & TANGLED) != 0u) {
        return x;
    }
    float3 origin = job.epoch_start[i].xyz;
    for (uint k = 0u; k < CLOTH_CANDIDATES; ++k) {
        uint entry = job.contacts[base + CLOTH_WORD + k];
        if (entry == NONE) {
            break;
        }
        float gap = asfloat(job.contacts[base + GAP_WORD + k]);
        if (!(gap > 0.0)) {
            continue;
        }
        float3 n = unpack_direction(job.contacts[base + PLANE_WORD + k]);
        float approach = dot(origin - x, n);
        float allowed = DAT_SHARE * gap;
        if (approach > allowed) {
            x += n * (approach - allowed);
        }
    }
    return x;
}

struct BodyContact {
    float3 x;
    float3 corrected;
    float3 settled;
    float3 normal;
    float touch;
    bool pushed;
};

BodyContact body_contact(Trait trait, uint i, float3 x, float3 start, float3 corrected, float alpha, float before, float h, float friction) {
    uint base = i * CONTACT_WORDS;
    float radius = trait.b.w;
    BodyContact contact = BodyContact(x, float3(0.0), float3(0.0), float3(0.0), 0.0, false);
    if (radius < 0.0) {
        return contact;
    }
    float nearest = asfloat(job.contacts[base + NEAREST_WORD]);
    float3 origin = job.epoch_start[i].xyz;
    float drift = distance(x, origin);
    bool outside = false;
    if (nearest < FOREVER) {
        float spans[BODY_CANDIDATES];
        float planes[BODY_CANDIDATES];
        uint entries[BODY_CANDIDATES];
        float3 facings[BODY_CANDIDATES];
        for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
            uint entry = job.contacts[base + k];
            entries[k] = entry;
            spans[k] = FOREVER;
            planes[k] = 0.0;
            if (entry == NONE) {
                continue;
            }
            float4 g4 = job.geometry[(i * BODY_CANDIDATES + k) * GEOMETRY_VECS + 4u];
            if (g4.z - drift - g4.w > radius) {
                continue;
            }
            float3 a0;
            float3 b0;
            float3 c0;
            float3 a1;
            float3 b1;
            float3 c1;
            cached_body(i, k, g4, a0, b0, c0, a1, b1, c1);
            float3 a = lerp(a0, a1, alpha);
            float3 b = lerp(b0, b1, alpha);
            float3 c = lerp(c0, c1, alpha);
            float3 n = face_normal(a, b, c);
            if (all(n == float3(0.0))) {
                continue;
            }
            spans[k] = distance(x, closest_on_triangle(x, a, b, c).point);
            planes[k] = dot(x - a, n);
            facings[k] = n;
        }
        uint overall = 0u;
        for (uint k = 1u; k < BODY_CANDIDATES; ++k) {
            bool tied = abs(spans[k] - spans[overall]) <= SAME_SPAN;
            if ((!tied && spans[k] < spans[overall]) || (tied && abs(planes[k]) > abs(planes[overall]))) {
                overall = k;
            }
        }
        float snug = radius;
        for (uint k = 0u; k < BODY_CANDIDATES && spans[overall] < radius; ++k) {
            if (!(spans[k] < radius)) {
                continue;
            }
            for (uint j = 0u; j < BODY_CANDIDATES; ++j) {
                if (j != k && spans[j] < 2.0 * radius - spans[k] && dot(facings[k], facings[j]) < PINCH_FACING) {
                    snug = min(snug, 0.5 * (spans[k] + spans[j]));
                }
            }
        }
        snug = max(snug, min(radius, SNUG_MIN));
        if (spans[overall] < FOREVER) {
            uint flags = job.contacts[base + FLAG_WORD];
            float margin = VERDICT_MARGIN * radius;
            outside = (flags & JUDGED_INSIDE) != 0u ? planes[overall] >= margin : planes[overall] > -margin;
            uint judged = (flags & ~JUDGED_INSIDE) | (outside ? 0u : JUDGED_INSIDE);
            if (judged != flags) {
                job.contacts[base + FLAG_WORD] = judged;
            }
        }
        float3 push_normals[BODY_CANDIDATES];
        float push_depths[BODY_CANDIDATES];
        float settled_depths[BODY_CANDIDATES];
        float pushes = 0.0;
        uint pushing = 0u;
        float3 lone = float3(0.0);
        float3 lone_settled = float3(0.0);
        float closest = FOREVER;
        float3 travel = float3(0.0);
        float3 contact_normal = float3(0.0);
        float deepest = 0.0;
        uint carried_bits = job.contacts[base + RESIDUAL_WORD];
        float carried = carried_bits == NONE ? FOREVER : asfloat(carried_bits);
        float residual = 0.0;
        uint judged_figure = NONE;
        uint verdict = 0u;
        for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
            push_depths[k] = 0.0;
            settled_depths[k] = 0.0;
            uint entry = entries[k];
            if (entry == NONE || spans[k] == FOREVER) {
                continue;
            }
            uint figure = obstacle_figure(entry);
            if (figure != judged_figure) {
                judged_figure = figure;
                verdict = k;
                for (uint j = 0u; j < BODY_CANDIDATES; ++j) {
                    if (entries[j] == NONE || obstacle_figure(entries[j]) != figure) {
                        continue;
                    }
                    bool tied = abs(spans[j] - spans[verdict]) <= SAME_SPAN;
                    if ((!tied && spans[j] < spans[verdict]) || (tied && abs(planes[j]) > abs(planes[verdict]))) {
                        verdict = j;
                    }
                }
            }
            if (planes[verdict] >= 0.0 && planes[k] >= snug && spans[k] >= snug) {
                closest = min(closest, min(planes[k], spans[k]));
                continue;
            }
            float4 g4 = job.geometry[(i * BODY_CANDIDATES + k) * GEOMETRY_VECS + 4u];
            float3 a0;
            float3 b0;
            float3 c0;
            float3 a1;
            float3 b1;
            float3 c1;
            cached_body(i, k, g4, a0, b0, c0, a1, b1, c1);
            float3 a = lerp(a0, a1, alpha);
            float3 b = lerp(b0, b1, alpha);
            float3 c = lerp(c0, c1, alpha);
            float3 pa = lerp(a0, a1, before);
            float3 pb = lerp(b0, b1, before);
            float3 pc_ = lerp(c0, c1, before);
            uint seen = entry;
            if (planes[verdict] >= 0.0) {
                if (planes[k] >= 0.0) {
                    seen = (entry & OBSTACLE_MASK) | REMEMBERED | ABOVE;
                } else {
                    float3 shift = ((a - a0) + (b - b0) + (c - c0)) / 3.0;
                    float3 sub_shift = ((a - pa) + (b - pb) + (c - pc_)) / 3.0;
                    if (!segment_crosses(origin + shift, x, a, b, c)
                        && !segment_crosses(start + sub_shift, x, a, b, c)) {
                        continue;
                    }
                }
            }
            Gap g = gap_to(x, a, b, c, seen);
            if (!g.valid) {
                continue;
            }
            closest = min(closest, g.gap);
            if (g.gap >= snug) {
                continue;
            }
            float depth = snug - g.gap;
            Gap was = gap_to(start, pa, pb, pc_, seen);
            float existing = was.valid ? max(snug - was.gap, 0.0) : 0.0;
            float prior = min(existing, carried);
            float allowed = depth - max(prior - PUSH_SPEED * h, 0.0);
            residual = max(residual, depth - max(allowed, 0.0));
            if (allowed <= 0.0) {
                continue;
            }
            push_normals[k] = g.normal;
            push_depths[k] = allowed;
            settled_depths[k] = min(existing, allowed);
            pushes += allowed;
            pushing += 1u;
            lone = g.normal * allowed;
            lone_settled = g.normal * settled_depths[k];
            if (depth > deepest) {
                deepest = depth;
                contact_normal = g.normal;
                float3 now_point = g.weights.x * a + g.weights.y * b + g.weights.z * c;
                float3 then_point = g.weights.x * pa + g.weights.y * pb + g.weights.z * pc_;
                travel = now_point - then_point;
            }
        }
        if (pushes > 0.0) {
            float3 moved = pushing == 1u ? lone : float3(0.0);
            float3 settled = pushing == 1u ? lone_settled : float3(0.0);
            for (uint k = 0u; k < BODY_CANDIDATES && pushing > 1u; ++k) {
                if (!(push_depths[k] > 0.0)) {
                    continue;
                }
                float overlap = 1.0;
                for (uint j = 0u; j < BODY_CANDIDATES; ++j) {
                    if (j != k && push_depths[j] > 0.0) {
                        overlap += abs(dot(push_normals[k], push_normals[j])) * min(push_depths[j] / OVERLAP_DEPTH, 1.0);
                    }
                }
                moved += push_normals[k] * (push_depths[k] / overlap);
                settled += push_normals[k] * (settled_depths[k] / overlap);
            }
            x += moved;
            contact.normal = contact_normal;
            contact.corrected = undoing(corrected, contact_normal, dot(moved, contact_normal));
            contact.settled = settled;
            float3 slid = (x - start) - travel;
            float3 tangential = slid - contact_normal * dot(slid, contact_normal);
            float slide = length(tangential);
            if (slide > 0.0 && friction > 0.0) {
                x -= tangential * min(friction * deepest / slide, 1.0);
            }
            closest = min(closest, snug);
            contact.pushed = true;
        }
        contact.touch = clamp(1.0 - (closest - snug) / BAND, 0.0, 1.0);
        if (residual != carried) {
            job.contacts[base + RESIDUAL_WORD] = asuint(residual);
        }
    }
    uint packed = job.contacts[base + COLLIDER_WORD];
    bool rescued = false;
    float rescue_cap = max(PUSH_SPEED * h, RESCUE_STEP);
    for (uint k = 0u; k < 2u; ++k) {
        uint index = (packed >> (16u * k)) & 0xffffu;
        if (index == NO_COLLIDER) {
            continue;
        }
        Collider collider = collider_between(job.colliders_from, job.colliders_to, index, alpha);
        float deep = collider.shape.x == 0u ? FOREVER : collider_limit(outside, asfloat(collider.shape.w));
        float2 found = collider_distance_past(collider, job.planes_to, x, deep);
        float limit = collider_limit(outside, found.y);
        if (found.x < limit) {
            float3 exit = collider_exit(collider, job.colliders_from[index], job.planes_to, job.planes_from, x, origin, limit);
            float out_by = length(exit);
            if (out_by > 1.0e-9) {
                float applied = min(out_by, rescue_cap);
                float3 step_exit = exit * (applied / out_by);
                contact.corrected += undoing(corrected + contact.corrected, exit / out_by, applied);
                contact.settled += step_exit;
                x += step_exit;
                contact.normal += step_exit;
                rescued = true;
                contact.pushed = true;
                contact.touch = 1.0;
            }
        }
    }
    if (rescued) {
        for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
            uint entry = job.contacts[base + k];
            if (entry == NONE) {
                continue;
            }
            float4 g4 = job.geometry[(i * BODY_CANDIDATES + k) * GEOMETRY_VECS + 4u];
            float3 a0;
            float3 b0;
            float3 c0;
            float3 a1;
            float3 b1;
            float3 c1;
            cached_body(i, k, g4, a0, b0, c0, a1, b1, c1);
            float3 a = lerp(a0, a1, alpha);
            float3 n = face_normal(a, lerp(b0, b1, alpha), lerp(c0, c1, alpha));
            bool in_front = dot(x - a, n) >= 0.0;
            job.contacts[base + k] = (entry & OBSTACLE_MASK) | REMEMBERED | (in_front ? ABOVE : 0u);
        }
    }
    float normal_length = length(contact.normal);
    contact.normal = normal_length > 1.0e-9 ? contact.normal / normal_length : float3(0.0, 1.0, 0.0);
    contact.x = x;
    return contact;
}

void settle(float alpha, float before, float h) {
    for (uint i = first + lane; i < first + count; i += THREADS) {
        Trait trait = job.traits[i];
        if (trait.a.x == 0.0) {
            continue;
        }
        float4 pred = job.preds[i];
        float touch = pred.w;
        float4 self_push = job.selfs[i];
        float3 self_only = self_push.xyz;
        float self_length = length(self_push.xyz);
        float3 self_normal = self_length > 1.0e-12 ? self_push.xyz / self_length : float3(0.0);
        float3 wrapped;
        float3 x = position(i) + gathered(i, wrapped);
        self_push.xyz += wrapped;
        x += self_push.xyz + extended(i, x, alpha);
        x = tethered(i, x, alpha);
        Piece piece = job.pieces[trait.d.x];
        float3 barrier;
        x = limited(trait, i, x, alpha, h, 1.0 - clamp(touch, 0.0, 1.0), clamp(piece.grab.x, 0.0, 1.0), barrier);
        float3 free = x;
        x = truncated(i, x, alpha);
        float3 trunc_push = x - free;
        float3 start = job.starts[i].xyz + wrapped;
        float3 corrected = free - self_push.xyz - pred.xyz;
        float3 self_undoing = undoing(corrected, self_normal, self_length);
        BodyContact contact = body_contact(trait, i, x, start, corrected + self_undoing, alpha, before, h, piece.contact.y);
        uint partner = asuint(self_push.w);
        bool inner_hit = false;
        if (self_length > 1.0e-9 && partner != NONE && piece.contact.x > 0.5 && piece.layer_triangles.x > 0u) {
            uint entry = job.contacts[i * CONTACT_WORDS + CLOTH_WORD + partner];
            inner_hit = obstacle_figure(entry) == job.shapes_figure.z && obstacle_index(entry) < piece.layer_triangles.x;
        }
        float touch_now = inner_hit ? max(contact.touch, 0.5) : contact.touch;
        job.preds[i] = float4(pred.xyz, lerp(touch_now, touch, exp(-h / TOUCH_FADE)));
        float3 barrier_settled = barrier * BARRIER_SETTLE;
        job.selfs[i] = float4(self_push.xyz - self_undoing + barrier_settled + contact.x - free - contact.corrected, self_push.w);
        place(i, contact.x);
        float wrap = length(wrapped);
        float wrap_touch = wrap > 0.0 ? 1.0 : 0.0;
        bool wrapping = !contact.pushed && wrap > 0.0;
        float touching = contact.pushed ? 1.0 : (wrap_touch > 0.0 ? wrap_touch : (inner_hit ? 1.0 : 0.0));
        float3 press_dir = contact.pushed ? contact.normal : (wrapping ? wrapped / wrap : self_normal);
        float pressed = touching > 0.0 ? pressed_code(press_dir, touching) : 0.0;
        job.starts[i] = float4(start + contact.settled + barrier_settled + (self_only + trunc_push) * SELF_SETTLE, pressed);
    }
}

void post_project(float alpha, bool with_mesh) {
    for (uint i = first + lane; i < first + count; i += THREADS) {
        Trait trait = job.traits[i];
        if (trait.a.x == 0.0) {
            continue;
        }
        float radius = trait.b.w;
        uint base = i * CONTACT_WORDS;
        float3 x = position(i);
        float3 moved = float3(0.0);
        if (radius >= 0.0) {
            if (with_mesh) {
                float nearest = asfloat(job.contacts[base + NEAREST_WORD]);
                if (nearest < FOREVER) {
                    float3 origin = job.epoch_start[i].xyz;
                    float drift = distance(x, origin);
                    float3 push = float3(0.0);
                    float pushes = 0.0;
                    float deepest = 0.0;
                    for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
                        uint entry = job.contacts[base + k];
                        if (entry == NONE || (entry & (REMEMBERED | ABOVE)) != (REMEMBERED | ABOVE)) {
                            continue;
                        }
                        float4 g4 = job.geometry[(i * BODY_CANDIDATES + k) * GEOMETRY_VECS + 4u];
                        if (g4.z - drift - g4.w > radius) {
                            continue;
                        }
                        float3 a;
                        float3 b;
                        float3 c;
                        cached_body_end(i, k, g4, a, b, c);
                        Gap g = gap_to(x, a, b, c, entry);
                        if (!g.valid || g.gap >= 0.0) {
                            continue;
                        }
                        float depth = -g.gap;
                        if (depth > POST_MESH_MIN && depth < POST_MESH_MAX) {
                            push += g.normal * depth;
                            pushes += depth;
                            deepest = max(deepest, depth);
                        }
                    }
                    if (pushes > 0.0) {
                        float amount = min(deepest, POST_STEP);
                        float3 delta = push * (amount / pushes);
                        x += delta;
                        moved += delta;
                    }
                }
            }
            uint packed = job.contacts[base + COLLIDER_WORD];
            if (packed != NONE) {
                uint flags = job.contacts[base + FLAG_WORD];
                bool outside = flags == NONE || (flags & JUDGED_INSIDE) == 0u;
                for (uint k = 0u; k < 2u; ++k) {
                    uint index = (packed >> (16u * k)) & 0xffffu;
                    if (index == NO_COLLIDER) {
                        continue;
                    }
                    Collider collider = with_mesh
                        ? job.colliders_to[index]
                        : collider_between(job.colliders_from, job.colliders_to, index, alpha);
                    float3 outward;
                    float2 found = collider_distance(collider, job.planes_to, x, outward);
                    float limit = collider_limit(outside, found.y);
                    if (found.x < limit) {
                        float amount = min(limit - found.x, POST_STEP);
                        float3 delta = outward * amount;
                        x += delta;
                        moved += delta;
                    }
                }
            }
        }
        bool body_moved = length(moved) > 1.0e-9;
        float4 start_rec = job.starts[i];
        Piece piece = job.pieces[trait.d.x];
        if (piece.contact.x > 0.5) {
            float thick = trait.b.z;
            bool body_free = !body_moved && start_rec.w == 0.0;
            float3 push = float3(0.0);
            float pushes = 0.0;
            float deepest = 0.0;
            bool mark_pressed = false;
            for (uint k = 0u; k < CLOTH_CANDIDATES; ++k) {
                uint entry = job.contacts[base + CLOTH_WORD + k];
                if (entry == NONE) {
                    break;
                }
                if (obstacle_figure(entry) != job.shapes_figure.z) {
                    continue;
                }
                uint idx = obstacle_index(entry);
                bool own_inner = idx < piece.layer_triangles.x && (entry & REMEMBERED) != 0u;
                bool own_outer = idx >= piece.layer_triangles.y && (entry & (REMEMBERED | ABOVE)) == REMEMBERED && body_free;
                if (!own_inner && !own_outer) {
                    continue;
                }
                uint4 corners = job.triangles[idx];
                if (job.pieces[corners.w].contact.x < 0.5) {
                    continue;
                }
                float3 a;
                float3 b;
                float3 c;
                own_layer_triangle(corners, own_outer, alpha, a, b, c);
                Gap g = gap_to(x, a, b, c, entry);
                if (!g.valid) {
                    continue;
                }
                float layer_t = max(snug_thickness(i, corners, thick), 0.0);
                float target_gap = own_inner ? min(0.5 * thick, layer_t) : layer_t;
                if (g.gap < target_gap) {
                    float depth = target_gap - g.gap;
                    if (depth > POST_MESH_MIN && depth < POST_MESH_MAX) {
                        push += g.normal * depth;
                        pushes += depth;
                        deepest = max(deepest, depth);
                        mark_pressed = mark_pressed || own_inner;
                    }
                }
            }
            if (pushes > 0.0) {
                float amount = min(deepest, POST_LAYER_STEP);
                float3 delta = push * (amount / pushes);
                x += delta;
                moved += delta;
                float delta_len = length(delta);
                if (mark_pressed && delta_len > 1.0e-9) {
                    start_rec.w = pressed_code(delta / delta_len, 1.0);
                    job.starts[i] = start_rec;
                }
            }
        }
        float moved_len = length(moved);
        if (moved_len > 1.0e-9) {
            float3 dir = moved / moved_len;
            place(i, x);
            halt(i, dir);
            if (body_moved && start_rec.w == 0.0) {
                start_rec.w = pressed_code(dir, 1.0);
                job.starts[i] = start_rec;
            }
        }
    }
}

void grip(float alpha, bool firm_only) {
    float substeps = float(job.steps.z);
    for (uint at = lane; at < job.flags.w; at += THREADS) {
        uint4 entry = job.grabbed[at];
        uint i = entry.x;
        if (!mine(i)) {
            continue;
        }
        float4 from = job.grabs_from[entry.z];
        float4 to = job.grabs_to[entry.z];
        float strength = clamp(lerp(from.w, to.w, alpha) * asfloat(entry.w), 0.0, 1.0);
        if (!(strength > 0.0) || (firm_only && strength < 1.0)) {
            continue;
        }
        float share = strength >= 1.0 ? 1.0 : 1.0 - pow(1.0 - strength, 1.0 / substeps);
        float3 grabbed = lerp(from.xyz, to.xyz, alpha) + target(i, alpha) - target(entry.y, alpha);
        place(i, lerp(position(i), grabbed, share));
    }
}
