import cloth.common;
import cloth.world;
import cloth.job;

static const uint LANES = 16u;
static const uint PARTICLES = 4u;
static const uint MERGED = BODY_CANDIDATES + CLOTH_CANDIDATES;

groupshared float merged_keys[PARTICLES][LANES][MERGED];
groupshared uint merged_ids[PARTICLES][LANES][MERGED];
groupshared float collider_keys[PARTICLES][LANES][2];
groupshared uint collider_ids[PARTICLES][LANES][2];
groupshared uint previous[PARTICLES][MERGED];
groupshared float4 sighted[PARTICLES][BODY_CANDIDATES];
groupshared float sighted_distance[PARTICLES][BODY_CANDIDATES];

float merged_keys_at(uint slot, uint lane, uint k) {
    return shared_load(__getAddress(merged_keys[slot][lane][k]));
}

void set_merged_keys(uint slot, uint lane, uint k, float value) {
    shared_store(__getAddress(merged_keys[slot][lane][k]), value);
}

uint merged_ids_at(uint slot, uint lane, uint k) {
    return shared_load(__getAddress(merged_ids[slot][lane][k]));
}

void set_merged_ids(uint slot, uint lane, uint k, uint value) {
    shared_store(__getAddress(merged_ids[slot][lane][k]), value);
}

float collider_keys_at(uint slot, uint lane, uint k) {
    return shared_load(__getAddress(collider_keys[slot][lane][k]));
}

void set_collider_keys(uint slot, uint lane, uint k, float value) {
    shared_store(__getAddress(collider_keys[slot][lane][k]), value);
}

uint collider_ids_at(uint slot, uint lane, uint k) {
    return shared_load(__getAddress(collider_ids[slot][lane][k]));
}

void set_collider_ids(uint slot, uint lane, uint k, uint value) {
    shared_store(__getAddress(collider_ids[slot][lane][k]), value);
}

uint previous_at(uint slot, uint k) {
    return shared_load(__getAddress(previous[slot][k]));
}

void set_previous(uint slot, uint k, uint value) {
    shared_store(__getAddress(previous[slot][k]), value);
}

float4 sighted_at(uint slot, uint k) {
    return shared_load(__getAddress(sighted[slot][k]));
}

void set_sighted(uint slot, uint k, float4 value) {
    shared_store(__getAddress(sighted[slot][k]), value);
}

float sighted_distance_at(uint slot, uint k) {
    return shared_load(__getAddress(sighted_distance[slot][k]));
}

void set_sighted_distance(uint slot, uint k, float value) {
    shared_store(__getAddress(sighted_distance[slot][k]), value);
}

static float best[2];
static uint chosen[2];

void keep_collider(float d, uint k) {
    bool first = d < best[0] || (d == best[0] && k < chosen[0]);
    bool second = d < best[1] || (d == best[1] && k < chosen[1]);
    if (first) {
        best[1] = best[0];
        chosen[1] = chosen[0];
        best[0] = d;
        chosen[0] = k;
    } else if (second) {
        best[1] = d;
        chosen[1] = k;
    }
}

struct Push {
    Jobs jobs;
    uint job;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const float SAME_SHEET = 0.02;
static const float CORRECT_DEPTH = 0.05;
static const float HELD_REACH = 0.04;
static const float SAME_DISTANCE = 1.0e-5;
static const float SQUEEZE = 0.015;
static const float KEPT_BONUS = 0.003;
static const float OPPOSED = 0.3;
static const float OPPOSED_KEY = 0.6;
static const uint INSIDE = 1u;
static const uint LEAVING = 2u;
static const uint MAX_CELLS = 4u;
static const float TRAVEL_MARGIN = 0.002;
static const float MAX_PROBE_TRAVEL = 0.06;
static const float INNER_MARGIN = 0.018;
static const float INNER_KEY_BONUS = 0.012;
static const float INNER_REST_RANGE = 0.06;
static const float OUTER_REST_RANGE = 0.08;
static const float OUTER_CONTAIN_DEPTH = 0.45;
static Job job;
static uint own;
static uint asking;
static float3 facing;

static float body_keys[BODY_CANDIDATES];
static uint body_ids[BODY_CANDIDATES];
static float cloth_keys[CLOTH_CANDIDATES];
static uint cloth_ids[CLOTH_CANDIDATES];

bool earlier(float key, uint id, float other_key, uint other_id) {
    return key < other_key || (key == other_key && id < other_id);
}

void keep_body(float key, uint id) {
    [ForceUnroll]
    for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
        bool repeated = id == body_ids[k];
        bool before = key < body_keys[k] || (key == body_keys[k] && id < body_ids[k]);
        float kept_key = body_keys[k];
        uint kept_id = body_ids[k];
        body_keys[k] = before ? key : kept_key;
        body_ids[k] = before ? id : kept_id;
        key = repeated ? FOREVER : (before ? kept_key : key);
        id = repeated ? NONE : (before ? kept_id : id);
    }
}

void keep_cloth(float key, uint id) {
    [ForceUnroll]
    for (uint k = 0u; k < CLOTH_CANDIDATES; ++k) {
        bool repeated = id == cloth_ids[k];
        bool before = key < cloth_keys[k] || (key == cloth_keys[k] && id < cloth_ids[k]);
        float kept_key = cloth_keys[k];
        uint kept_id = cloth_ids[k];
        cloth_keys[k] = before ? key : kept_key;
        cloth_ids[k] = before ? id : kept_id;
        key = repeated ? FOREVER : (before ? kept_key : key);
        id = repeated ? NONE : (before ? kept_id : id);
    }
}

void cloth_corners(uint figure, uint triangle, out float3 a, out float3 b, out float3 c, out uint4 corners) {
    ReadVec4s start = job.figures[figure].epoch_start;
    corners = job.figures[figure].triangles[triangle];
    a = start[corners.x].xyz;
    b = start[corners.y].xyz;
    c = start[corners.z].xyz;
}

bool neighbouring(uint i, uint4 corners) {
    if (corners.x == i || corners.y == i || corners.z == i) {
        return true;
    }
    uint to = job.incident_starts[i + 1u];
    for (uint at = job.incident_starts[i]; at < to; ++at) {
        uint4 near = job.triangles[job.incident[at]];
        if (any(near.xyzz == corners.xxxx) || any(near.xyzz == corners.yyyy)
            || any(near.xyzz == corners.zzzz)) {
            return true;
        }
    }
    return false;
}

uint previous_bits(uint slot, uint id, uint from, uint count) {
    for (uint k = 0u; k < count; ++k) {
        uint entry = previous_at(slot, from + k);
        if (entry != NONE && (entry & OBSTACLE_MASK) == id) {
            return (entry & REMEMBERED) != 0u ? entry & (ABOVE | REMEMBERED) : NONE;
        }
    }
    return NONE;
}
float body_key(float d, uint id, float3 n, float plane) {
    float key = dot(n, facing) < -OPPOSED && plane >= 0.0 ? OPPOSED_KEY * d : d;
    for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
        uint entry = previous_at(asking, k);
        if (entry != NONE && (entry & OBSTACLE_MASK) == id) {
            return key - KEPT_BONUS;
        }
    }
    return key;
}


float body_eval(float3 x, float3 x_end, uint figure, uint index, uint id, out float key) {
    float3 a0;
    float3 b0;
    float3 c0;
    float3 a1;
    float3 b1;
    float3 c1;
    body_endpoints(job.figures, figure, index, a0, b0, c0, a1, b1, c1);
    float d = min(distance(x, closest_on_triangle(x, a0, b0, c0).point),
        distance(x_end, closest_on_triangle(x_end, a1, b1, c1).point));
    float3 xm = 0.5 * (x + x_end);
    float3 am = 0.5 * (a0 + a1);
    float3 bm = 0.5 * (b0 + b1);
    float3 cm = 0.5 * (c0 + c1);
    d = min(d, distance(xm, closest_on_triangle(xm, am, bm, cm).point));
    d = min(d, distance(x, closest_on_triangle(x, a1, b1, c1).point));
    float3 n1 = face_normal(a1, b1, c1);
    key = body_key(d, id, n1, dot(x_end - a1, n1));
    return d;
}

void hold_previous(float3 x, float3 x_end, uint entry, float reach) {
    if (entry == NONE) {
        return;
    }
    uint id = entry & OBSTACLE_MASK;
    uint figure = obstacle_figure(id);
    uint index = obstacle_index(id);
    if (obstacle_is_cloth(id) || figure >= job.flags.z
        || index >= job.figures[figure].counts.w) {
        return;
    }
    float key;
    float d = body_eval(x, x_end, figure, index, id, key);
    if (d <= reach && earlier(key, id, body_keys[BODY_CANDIDATES - 1u], body_ids[BODY_CANDIDATES - 1u])) {
        keep_body(key, id);
    }
}

uint2 sighted_bits(uint slot, uint lane, uint figure, float3 n, float plane, float h) {
    float nearest = FOREVER;
    for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
        uint id = merged_ids_at(slot, 0, k);
        if (id != NONE && obstacle_figure(id) == figure) {
            nearest = min(nearest, sighted_distance_at(slot, k));
        }
    }
    float steepest = -1.0;
    float4 exit = float4(n, plane);
    for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
        uint id = merged_ids_at(slot, 0, k);
        float4 seen = sighted_at(slot, k);
        if (id != NONE && obstacle_figure(id) == figure
            && sighted_distance_at(slot, k) <= nearest + SAME_DISTANCE && abs(seen.w) > steepest) {
            steepest = abs(seen.w);
            exit = seen;
        }
    }
    bool inside = exit.w < 0.0;
    bool above = plane >= -h || (inside && (dot(n, exit.xyz) > 0.0 || plane >= -max(h, SQUEEZE)));
    bool leaving = inside && sighted_distance_at(slot, lane) <= nearest + SAME_DISTANCE;
    return uint2(REMEMBERED | (above ? ABOVE : 0u), (inside ? INSIDE : 0u) | (leaving ? LEAVING : 0u));
}

uint side_bits(uint previous, uint2 sighting, float plane, float h) {
    if (previous == NONE) {
        return sighting.x;
    }
    if ((sighting.y & INSIDE) != 0u) {
        return (sighting.y & LEAVING) != 0u ? (REMEMBERED | ABOVE) : previous;
    }
    if (plane >= 0.0) {
        return REMEMBERED | ABOVE;
    }
    return plane < -max(h, SQUEEZE) ? REMEMBERED : previous;
}

struct Query {
    float3 x;
    float3 x_end;
    float3 rest;
    uint particle;
    float body_reach;
    float cloth_reach;
    float inner_reach;
    float thick;
    bool is_sheet;
    bool layered_sheet;
    bool self_colliding;
    uint sheet;
    uint2 layer_triangles;
};

float3 swept_end(uint particle, float3 start_pos, bool own_inner) {
    if (!own_inner && job.traits[particle].a.x == 0.0) {
        return asfloat(job.targets_to[particle].xyz);
    }
    return start_pos + clamp_length(job.epoch_motion[particle].xyz * job.step.w, MAX_SWEEP_STEP);
}

float layer_cloth_eval(float3 x, float3 x_end, uint4 corners, bool own_inner, out float key) {
    float3 a0 = job.epoch_start[corners.x].xyz;
    float3 b0 = job.epoch_start[corners.y].xyz;
    float3 c0 = job.epoch_start[corners.z].xyz;
    float3 a1 = swept_end(corners.x, a0, own_inner);
    float3 b1 = swept_end(corners.y, b0, own_inner);
    float3 c1 = swept_end(corners.z, c0, own_inner);
    float d = min(distance(x, closest_on_triangle(x, a0, b0, c0).point),
        distance(x_end, closest_on_triangle(x_end, a1, b1, c1).point));
    float3 xm = 0.5 * (x + x_end);
    d = min(d, distance(xm, closest_on_triangle(xm, 0.5 * (a0 + a1), 0.5 * (b0 + b1), 0.5 * (c0 + c1)).point));
    float cover = 0.0;
    if (own_inner) {
        ReadVec4s rest = job.rest;
        cover = max(max(rest[corners.x].w, rest[corners.y].w), rest[corners.z].w);
    }
    key = d - INNER_KEY_BONUS;
    return d - cover;
}

bool outer_upper(uint4 corners) {
    return asfloat(job.traits[corners.x].d.w) < OUTER_CONTAIN_DEPTH
        || asfloat(job.traits[corners.y].d.w) < OUTER_CONTAIN_DEPTH
        || asfloat(job.traits[corners.z].d.w) < OUTER_CONTAIN_DEPTH;
}

bool rest_under_outer(float3 at, uint4 corners) {
    Snug snug = snug_layer(job.rest, corners, at, 0.0);
    return snug.valid && !snug.above && snug.rest_distance < OUTER_REST_RANGE;
}

bool outer_holds(float3 rest_at, uint4 corners) {
    return outer_upper(corners) && rest_under_outer(rest_at, corners);
}

void offer_layer(float3 x, float3 x_end, uint4 corners, bool own_inner, uint id, float reach) {
    float key;
    float d = layer_cloth_eval(x, x_end, corners, own_inner, key);
    if (d <= reach && earlier(key, id, cloth_keys[CLOTH_CANDIDATES - 1u], cloth_ids[CLOTH_CANDIDATES - 1u])) {
        keep_cloth(key, id);
    }
}

void hold_previous_layer(float3 x, float3 x_end, float3 rest_at, uint entry, uint2 layer_triangles, float reach) {
    if (entry == NONE) {
        return;
    }
    uint id = entry & OBSTACLE_MASK;
    uint index = obstacle_index(id);
    if (!obstacle_is_cloth(id) || obstacle_figure(id) != own) {
        return;
    }
    bool own_inner = index < layer_triangles.x;
    bool own_outer = index >= layer_triangles.y;
    if (!own_inner && !own_outer) {
        return;
    }
    uint4 corners = job.triangles[index];
    if (job.pieces[corners.w].contact.x < 0.5) {
        return;
    }
    if (own_outer && !outer_holds(rest_at, corners)) {
        return;
    }
    offer_layer(x, x_end, corners, own_inner, id, reach);
}

void consider(Query q, uint id) {
    uint figure = obstacle_figure(id);
    uint index = obstacle_index(id);
    if (!obstacle_is_cloth(id)) {
        if (q.body_reach < 0.0) {
            return;
        }
        float key;
        float d = body_eval(q.x, q.x_end, figure, index, id, key);
        if (d <= q.body_reach && earlier(key, id, body_keys[BODY_CANDIDATES - 1u], body_ids[BODY_CANDIDATES - 1u])) {
            keep_body(key, id);
        }
        return;
    }
    bool own_inner = figure == own && index < q.layer_triangles.x;
    bool own_outer = figure == own && index >= q.layer_triangles.y;
    bool own_beside = figure == own && !own_inner && !own_outer;
    if (own_beside && !q.self_colliding) {
        return;
    }
    uint4 corners = job.figures[figure].triangles[index];
    if (own_beside && job.pieces[corners.w].contact.w < 0.5) {
        return;
    }
    if (own_inner || own_outer) {
        bool sheet_pair = q.is_sheet && job.pieces[corners.w].contact.x > 0.5;
        if (own_outer && (!sheet_pair || !outer_holds(q.rest, corners))) {
            return;
        }
        if (sheet_pair) {
            offer_layer(q.x, q.x_end, corners, own_inner, id, q.inner_reach);
            return;
        }
    }
    float3 a;
    float3 b;
    float3 c;
    cloth_corners(figure, index, a, b, c, corners);
    float d = distance(q.x, closest_on_triangle(q.x, a, b, c).point);
    if (!own_beside) {
        ReadVec4s rest = job.figures[figure].rest;
        d -= max(max(rest[corners.x].w, rest[corners.y].w), rest[corners.z].w);
    }
    if (d > q.cloth_reach || !earlier(d, id, cloth_keys[CLOTH_CANDIDATES - 1u], cloth_ids[CLOTH_CANDIDATES - 1u])) {
        return;
    }
    if (own_beside) {
        if (neighbouring(q.particle, corners)) {
            return;
        }
        float3 ra = job.rest[corners.x].xyz;
        float3 rb = job.rest[corners.y].xyz;
        float3 rc = job.rest[corners.z].xyz;
        float apart = job.traits[corners.x].d.y == q.sheet ? SAME_SHEET : SNUG_LAYERS * q.thick;
        if (distance(q.rest, closest_on_triangle(q.rest, ra, rb, rc).point) < apart) {
            return;
        }
    }
    keep_cloth(d, id);
}

bool own_layer(Query q, uint id) {
    uint index = obstacle_index(id);
    return obstacle_figure(id) == own && (index < q.layer_triangles.x || index >= q.layer_triangles.y);
}

void walk(Query q, uint lane) {
    float cloth_walk = q.layered_sheet ? q.inner_reach : q.cloth_reach;
    float reach = max(q.body_reach, cloth_walk);
    float3 x_min = min(q.x, q.x_end);
    float3 x_max = max(q.x, q.x_end);
    float3 x_mid = 0.5 * (q.x + q.x_end);
    float half_span = 0.5 * distance(q.x, q.x_end);
    uint seen = 0u;
    for (uint level = 0u; level < LEVELS; ++level) {
        int3 low = grid_cell(x_min - float3(reach), level);
        int3 high = min(grid_cell(x_max + float3(reach), level), low + int3(MAX_CELLS - 1u));
        for (int cz = low.z; cz <= high.z; ++cz) {
            for (int cy = low.y; cy <= high.y; ++cy) {
                for (int cx = low.x; cx <= high.x; ++cx) {
                    uint cell = grid_hash(int3(cx, cy, cz), level);
                    uint start = job.cells[cell];
                    uint end = job.cells[cell + 1u];
                    uint offset = (lane + LANES - seen % LANES) % LANES;
                    for (uint e = start + offset; e < end; e += LANES) {
                        float4 sphere = job.spheres[e];
                        float apart = distance(x_mid, sphere.xyz) - abs(sphere.w) - half_span;
                        if (sphere.w >= 0.0) {
                            if (apart <= q.body_reach) {
                                consider(q, job.items[e]);
                            }
                        } else if (apart <= cloth_walk) {
                            uint item = job.items[e];
                            if (apart <= q.cloth_reach || own_layer(q, item)) {
                                consider(q, item);
                            }
                        }
                    }
                    seen += end - start;
                }
            }
        }
    }
    uint large = min(job.large[0], LARGE_OBSTACLES);
    for (uint k = lane; k < large; k += LANES) {
        consider(q, job.large[1u + k]);
    }
}

uint layered_side(uint i, uint cloth, uint4 corners, Piece piece, float thick) {
    if (obstacle_figure(cloth) != own) {
        return NONE;
    }
    uint index = obstacle_index(cloth);
    bool own_inner = index < piece.layer_triangles.x;
    bool own_outer = index >= piece.layer_triangles.y;
    if (!own_inner && !own_outer) {
        return NONE;
    }
    Snug snug = snug_layer(job.rest, corners, job.rest[i].xyz, thick);
    if (snug.in_range) {
        return REMEMBERED | (snug.above ? ABOVE : 0u);
    }
    if (snug.valid && piece.contact.x > 0.5 && job.pieces[corners.w].contact.x > 0.5) {
        if (own_inner && snug.rest_distance < INNER_REST_RANGE) {
            return REMEMBERED | (snug.above ? ABOVE : 0u);
        }
        if (own_outer && snug.rest_distance < OUTER_REST_RANGE && !snug.above) {
            return REMEMBERED;
        }
    }
    return NONE;
}

float target_need(float3 posed, float required, float alpha, uint id, out float3 push) {
    push = float3(0.0);
    float3 a;
    float3 b;
    float3 c;
    body_corners(job.figures, own, obstacle_index(id), alpha, a, b, c);
    float3 n = face_normal(a, b, c);
    if (all(n == float3(0.0)) || !closest_on_triangle(posed, a, b, c).face) {
        return 0.0;
    }
    float d = dot(posed - a, n);
    float need = required - d;
    if (d <= -CORRECT_DEPTH || !(need > 0.0)) {
        return 0.0;
    }
    push = n * need;
    return need;
}

void share(uint slot, uint lane) {
    set_collider_keys(slot, lane, 0, best[0]);
    set_collider_keys(slot, lane, 1, best[1]);
    set_collider_ids(slot, lane, 0, chosen[0]);
    set_collider_ids(slot, lane, 1, chosen[1]);
    [ForceUnroll]
    for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
        set_merged_keys(slot, lane, k, body_keys[k]);
        set_merged_ids(slot, lane, k, body_ids[k]);
    }
    [ForceUnroll]
    for (uint k = 0u; k < CLOTH_CANDIDATES; ++k) {
        set_merged_keys(slot, lane, BODY_CANDIDATES + k, cloth_keys[k]);
        set_merged_ids(slot, lane, BODY_CANDIDATES + k, cloth_ids[k]);
    }
}

void gather(uint slot, uint other) {
    for (uint k = 0u; k < 2u; ++k) {
        if (collider_ids_at(slot, other, k) != NO_COLLIDER) {
            keep_collider(collider_keys_at(slot, other, k), collider_ids_at(slot, other, k));
        }
    }
    for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
        uint id = merged_ids_at(slot, other, k);
        if (id != NONE) {
            keep_body(merged_keys_at(slot, other, k), id);
        }
    }
    for (uint k = 0u; k < CLOTH_CANDIDATES; ++k) {
        uint id = merged_ids_at(slot, other, BODY_CANDIDATES + k);
        if (id != NONE) {
            keep_cloth(merged_keys_at(slot, other, BODY_CANDIDATES + k), id);
        }
    }
}

void merge(uint slot, uint lane) {
    for (uint stride = 1u; stride < LANES; stride *= 2u) {
        share(slot, lane);
        group_barrier();
        if ((lane & (2u * stride - 1u)) == 0u) {
            gather(slot, lane + stride);
        }
        group_barrier();
    }
}

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 local : SV_GroupThreadID, uint3 group : SV_GroupID) {
    job = pc.jobs[pc.job];
    own = job.shapes_figure.z;
    uint slot = local.x / LANES;
    asking = slot;
    uint lane = local.x % LANES;
    uint i = group.x * PARTICLES + slot;
    bool present = i < job.range.y;
    Trait trait = {};
    bool searching = false;
    float3 x = float3(0.0);
    float h = -1.0;
    [ForceUnroll]
    for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
        body_keys[k] = FOREVER;
        body_ids[k] = NONE;
    }
    [ForceUnroll]
    for (uint k = 0u; k < CLOTH_CANDIDATES; ++k) {
        cloth_keys[k] = FOREVER;
        cloth_ids[k] = NONE;
    }
    best[0] = FOREVER;
    best[1] = FOREVER;
    chosen[0] = NO_COLLIDER;
    chosen[1] = NO_COLLIDER;
    Piece piece = {};
    uint base = i * CONTACT_WORDS;
    bool body_lane = lane < BODY_CANDIDATES;
    if (present) {
        trait = job.traits[i];
        searching = trait.a.x != 0.0;
        if (body_lane) {
            set_previous(slot, lane, job.contacts[base + lane]);
        }
        if (lane < CLOTH_CANDIDATES) {
            set_previous(slot, BODY_CANDIDATES + lane, job.contacts[base + CLOTH_WORD + lane]);
        }
    }
    group_barrier();
    if (searching) {
        x = job.epoch_start[i].xyz;
        piece = job.pieces[trait.d.x];
        facing = piece.contact.x > 0.5 && piece.contact.z < 0.5 ? job.normals[i].xyz : float3(0.0);
        h = trait.b.w;
        float thick = trait.b.z;
        float3 vel = job.velocities[i].xyz;
        float speed = length(vel);
        float step_len = speed * job.step.w;
        float3 step_move = vel * job.step.w;
        float3 x_end = step_len > MAX_SWEEP_STEP ? x + step_move * (MAX_SWEEP_STEP / step_len) : x + step_move;
        float3 x_mid = 0.5 * (x + x_end);
        float travel = min(step_len + TRAVEL_MARGIN, MAX_PROBE_TRAVEL);
        bool is_sheet = piece.contact.x > 0.5;
        bool layered_sheet = is_sheet && (piece.layer_triangles.x > 0u || piece.layer_triangles.y < job.range.w);
        float body_reach = h < 0.0 ? -1.0 : h + BAND + travel;
        float cloth_reach = thick + 0.5 * BAND + travel;
        float inner_reach = max(cloth_reach, thick + BAND + INNER_MARGIN + travel);
        walk(Query(x, x_end, job.rest[i].xyz, i, body_reach, cloth_reach, inner_reach, thick, is_sheet, layered_sheet, piece.contact.w > 0.5, trait.d.y, piece.layer_triangles.xy), lane);
        if (layered_sheet && lane < CLOTH_CANDIDATES) {
            hold_previous_layer(x, x_end, job.rest[i].xyz, previous_at(slot, BODY_CANDIDATES + lane), piece.layer_triangles.xy, inner_reach + HELD_REACH);
        }
        if (h >= 0.0) {
            if (body_lane) {
                hold_previous(x, x_end, previous_at(slot, lane), body_reach + HELD_REACH);
            }
            for (uint k = lane; k < job.shapes_figure.w; k += LANES) {
                Collider collider = job.colliders_to[k];
                float d0 = collider_distance_past(collider, job.planes_to, x, best[1]).x;
                float d1 = collider_distance_past(collider, job.planes_to, x_mid, min(best[1], d0)).x;
                keep_collider(min(d0, d1), k);
            }
        }
    }
    merge(slot, lane);
    if (lane == 0u) {
        [ForceUnroll]
        for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
            set_merged_keys(slot, 0, k, body_keys[k]);
            set_merged_ids(slot, 0, k, body_ids[k]);
        }
        [ForceUnroll]
        for (uint k = 0u; k < CLOTH_CANDIDATES; ++k) {
            set_merged_keys(slot, 0, BODY_CANDIDATES + k, cloth_keys[k]);
            set_merged_ids(slot, 0, BODY_CANDIDATES + k, cloth_ids[k]);
        }
        set_collider_ids(slot, 0, 0, chosen[0]);
        set_collider_ids(slot, 0, 1, chosen[1]);
    }
    group_barrier();
    float required = searching ? trait.c.z : 0.0;
    uint id = searching && body_lane ? merged_ids_at(slot, 0, lane) : NONE;
    float4 needs = float4(0.0);
    float3 from_push = float3(0.0);
    float3 to_push = float3(0.0);
    float3 a0 = float3(0.0);
    float3 b0 = float3(0.0);
    float3 c0 = float3(0.0);
    float3 n0 = float3(0.0);
    Closest start = Closest(float3(0.0), float3(0.0), false);
    if (body_lane) {
        set_sighted_distance(slot, lane, FOREVER);
    }
    if (id != NONE) {
        body_corners(job.figures, obstacle_figure(id), obstacle_index(id), 0.0, a0, b0, c0);
        n0 = face_normal(a0, b0, c0);
        start = closest_on_triangle(x, a0, b0, c0);
        set_sighted(slot, lane, float4(n0, dot(x - a0, n0)));
        set_sighted_distance(slot, lane, all(n0 == float3(0.0)) ? FOREVER : distance(x, start.point));
    }
    group_barrier();
    if (id != NONE) {
        uint2 sighting = sighted_bits(slot, lane, obstacle_figure(id), n0, sighted_at(slot, lane).w, h);
        uint bits = side_bits(previous_bits(slot, id, 0u, BODY_CANDIDATES), sighting, sighted_at(slot, lane).w, h);
        float3 a1;
        float3 b1;
        float3 c1;
        body_corners(job.figures, obstacle_figure(id), obstacle_index(id), 1.0, a1, b1, c1);
        job.contacts[base + lane] = id | bits;
        uint at = (i * BODY_CANDIDATES + lane) * GEOMETRY_VECS;
        job.geometry[at] = float4(a0, b0.x);
        job.geometry[at + 1u] = float4(b0.yz, c0.xy);
        job.geometry[at + 2u] = float4(c0.z, a1);
        job.geometry[at + 3u] = float4(b1, c1.x);
        float moved = max(max(distance(a0, a1), distance(b0, b1)), distance(c0, c1));
        job.geometry[at + 4u] = float4(c1.yz, merged_keys_at(slot, 0, lane), moved);
    } else if (searching && body_lane) {
        job.contacts[base + lane] = NONE;
    }
    uint2 winners = uint2(NONE);
    if (required > 0.0 && id != NONE && obstacle_figure(id) == own) {
        winners = uint2(id);
        if (job.flags.y != 0u) {
            needs.x = target_need(asfloat(job.targets_from[i].xyz), required, 0.0, id, from_push);
        }
        needs.y = target_need(asfloat(job.targets_to[i].xyz), required, 1.0, id, to_push);
    }
    bool tangled = false;
    if (searching && lane < CLOTH_CANDIDATES) {
        uint cloth = merged_ids_at(slot, 0, BODY_CANDIDATES + lane);
        float gap = -1.0;
        uint plane = 0u;
        if (cloth != NONE) {
            float3 a;
            float3 b;
            float3 c;
            uint4 corners;
            cloth_corners(obstacle_figure(cloth), obstacle_index(cloth), a, b, c, corners);
            float3 n = face_normal(a, b, c);
            float d = dot(x - a, n);
            uint flagged = job.contacts[base + FLAG_WORD];
            bool released = flagged != NONE && (flagged & TANGLED) != 0u;
            uint bits = layered_side(i, cloth, corners, piece, trait.b.z);
            if (bits == NONE && !released) {
                bits = previous_bits(slot, cloth, BODY_CANDIDATES, CLOTH_CANDIDATES);
            }
            Closest closest = closest_on_triangle(x, a, b, c);
            if (bits == NONE) {
                bits = closest.face ? (REMEMBERED | (d >= 0.0 ? ABOVE : 0u)) : 0u;
            }
            float side = (bits & ABOVE) != 0u ? 1.0 : (closest.face ? -1.0 : (d >= 0.0 ? 1.0 : -1.0));
            float3 away = x - closest.point;
            float apart = length(away);
            bool separated = closest.face ? side * d > 0.0 : apart > 1.0e-6;
            gap = separated ? apart : -1.0;
            plane = pack_direction(closest.face ? side * n : away / max(apart, 1.0e-9));
            uint idx = obstacle_index(cloth);
            bool layered_pair = obstacle_figure(cloth) == own
                && (idx < piece.layer_triangles.x || idx >= piece.layer_triangles.y)
                && piece.contact.x > 0.5
                && job.pieces[corners.w].contact.x > 0.5;
            if (obstacle_figure(cloth) == own && !layered_pair) {
                uint to = job.incident_starts[i + 1u];
                for (uint at = job.incident_starts[i]; at < to && !tangled; ++at) {
                    uint4 near = job.triangles[job.incident[at]];
                    uint others[3] = { near.x, near.y, near.z };
                    for (uint m = 0u; m < 3u; ++m) {
                        if (others[m] != i && segment_crosses(x, job.epoch_start[others[m]].xyz, a, b, c)) {
                            tangled = true;
                        }
                    }
                }
            }
            cloth |= bits;
        }
        job.contacts[base + CLOTH_WORD + lane] = cloth;
        job.contacts[base + GAP_WORD + lane] = asuint(gap);
        job.contacts[base + PLANE_WORD + lane] = plane;
    }
    group_barrier();
    set_collider_keys(slot, lane, 0, needs.x);
    set_collider_keys(slot, lane, 1, needs.y);
    set_merged_keys(slot, lane, 0, from_push.x);
    set_merged_keys(slot, lane, 1, from_push.y);
    set_merged_keys(slot, lane, 2, from_push.z);
    set_merged_keys(slot, lane, 3, to_push.x);
    set_merged_keys(slot, lane, 4, to_push.y);
    set_merged_keys(slot, lane, 5, to_push.z);
    set_merged_ids(slot, lane, 0, tangled ? 1u : 0u);
    set_merged_ids(slot, lane, 1, winners.x);
    set_merged_ids(slot, lane, 2, winners.y);
    sync();
    if (!present || lane != 0u) {
        return;
    }
    if (!searching) {
        for (uint k = 0u; k < CONTACT_WORDS; ++k) {
            job.contacts[base + k] = NONE;
        }
        for (uint k = 0u; k < NEAREST_WORDS; ++k) {
            job.nearest[NEAREST_WORDS * i + k] = NONE;
        }
        return;
    }
    uint nearest_count = 0u;
    bool knotted = false;
    float worst_from = 0.0;
    float worst_to = 0.0;
    uint from_winner = NONE;
    uint to_winner = NONE;
    float3 push_from = float3(0.0);
    float3 push_to = float3(0.0);
    for (uint k = 0u; k < LANES; ++k) {
        uint entry = k < BODY_CANDIDATES ? job.contacts[base + k] : NONE;
        bool facing = (entry & (REMEMBERED | ABOVE)) != REMEMBERED;
        if (entry != NONE && obstacle_figure(entry) == own && facing && nearest_count < BODY_NEAREST) {
            job.nearest[NEAREST_WORDS * i + nearest_count] = entry;
            nearest_count += 1u;
        }
        knotted = knotted || merged_ids_at(slot, k, 0) != 0u;
        if (collider_keys_at(slot, k, 0) > 0.0
            && earlier(-collider_keys_at(slot, k, 0), merged_ids_at(slot, k, 1), -worst_from, from_winner)) {
            worst_from = collider_keys_at(slot, k, 0);
            from_winner = merged_ids_at(slot, k, 1);
            push_from = float3(merged_keys_at(slot, k, 0), merged_keys_at(slot, k, 1), merged_keys_at(slot, k, 2));
        }
        if (collider_keys_at(slot, k, 1) > 0.0
            && earlier(-collider_keys_at(slot, k, 1), merged_ids_at(slot, k, 2), -worst_to, to_winner)) {
            worst_to = collider_keys_at(slot, k, 1);
            to_winner = merged_ids_at(slot, k, 2);
            push_to = float3(merged_keys_at(slot, k, 3), merged_keys_at(slot, k, 4), merged_keys_at(slot, k, 5));
        }
    }
    job.contacts[base + COLLIDER_WORD] = chosen[0] | (chosen[1] << 16u);
    uint flagged = job.contacts[base + FLAG_WORD];
    uint judged = flagged == NONE ? 0u : flagged & JUDGED_INSIDE;
    job.contacts[base + FLAG_WORD] = judged | (knotted ? TANGLED : 0u) | (best[0] < LIMB_REACH ? NEAR_LIMB : 0u);
    job.contacts[base + NEAREST_WORD] = asuint(body_keys[0]);
    for (uint k = nearest_count; k < BODY_NEAREST; ++k) {
        job.nearest[NEAREST_WORDS * i + k] = NONE;
    }
    uint layered_count = 0u;
    for (uint k = 0u; k < CLOTH_CANDIDATES; ++k) {
        uint entry = job.contacts[base + CLOTH_WORD + k];
        if (entry != NONE && obstacle_figure(entry) == own && obstacle_index(entry) < piece.layer_triangles.x) {
            job.nearest[NEAREST_WORDS * i + BODY_NEAREST + layered_count] = entry;
            layered_count += 1u;
        }
    }
    for (uint k = layered_count; k < LAYER_NEAREST; ++k) {
        job.nearest[NEAREST_WORDS * i + BODY_NEAREST + k] = NONE;
    }
    job.nearest[NEAREST_WORDS * i + COLLIDER_NEAREST] = chosen[0] | (chosen[1] << 16u);
    if (worst_from > 0.0 && job.flags.y != 0u) {
        uint4 target_from = job.targets_from[i];
        job.targets_from[i] = uint4(asuint(asfloat(target_from.xyz) + push_from), target_from.w);
    }
    if (worst_to > 0.0) {
        uint4 target_to = job.targets_to[i];
        job.targets_to[i] = uint4(asuint(asfloat(target_to.xyz) + push_to), target_to.w);
    }
}
