import cloth.common;
__exported import physics.triangle;

static const uint ABOVE = 0x80000000u;
static const uint REMEMBERED = 0x40000000u;
static const uint CLOTH = 0x20000000u;
static const uint FIGURE_SHIFT = 24u;
static const uint FIGURE_MASK = 0x1fu;
static const uint INDEX_MASK = 0x00ffffffu;
static const uint OBSTACLE_MASK = 0x3fffffffu;
static const uint BODY_CANDIDATES = 8u;
static const uint CLOTH_CANDIDATES = 4u;
static const uint WRAP_CANDIDATES = 4u;
static const uint CONTACT_WORDS = 24u;
static const uint CLOTH_WORD = 8u;
static const uint GAP_WORD = 12u;
static const uint PLANE_WORD = 16u;
static const uint COLLIDER_WORD = 20u;
static const uint FLAG_WORD = 21u;
static const uint NEAREST_WORD = 22u;
static const uint RESIDUAL_WORD = 23u;
static const uint BODY_NEAREST = 4u;
static const uint LAYER_NEAREST = 4u;
static const uint COLLIDER_NEAREST = BODY_NEAREST + LAYER_NEAREST;
static const uint NEAREST_WORDS = COLLIDER_NEAREST + 1u;
static const uint GEOMETRY_VECS = 5u;
static const uint TANGLED = 1u;
static const uint JUDGED_INSIDE = 2u;
static const uint NEAR_LIMB = 4u;
static const float LIMB_REACH = 0.06;
static const uint NO_COLLIDER = 0xffffu;
static const uint GRID_CELLS = 196608u;
static const uint LEVELS = 3u;
static const uint LEVEL_CELLS = 65536u;
static const float FINE_CELL = 0.025;
static const float MIDDLE_CELL = 0.05;
static const float COARSE_CELL = 0.12;
static const uint LARGE_OBSTACLES = 4096u;
static const float BAND = 0.01;
static const float FOREVER = 1.0e30;
static const float LATERAL_REACH = 0.004;
static const float SNUG_LAYERS = 2.0;
static const float MAX_SWEEP_STEP = 0.04;
struct Figure : INonWritable {
    ReadVec4s positions;
    ReadVec4s epoch_start;
    ReadVec4s rest;
    ReadVec4s traits;
    ReadUvec4s triangles;
    ReadVec4s pieces;
    ReadVec4s body_from;
    ReadVec4s body_to;
    ReadUvec4s body_triangles;
    ReadUints incident_starts;
    ReadUints incident;
    ReadVec4s epoch_motion;
    uint4 counts;

    static Figure load_from<let ALIGN : int>(Figure* address) {
        return {
            load_non_writable<8>(__getAddress(address->positions)),
            load_non_writable<8>(__getAddress(address->epoch_start)),
            load_non_writable<8>(__getAddress(address->rest)),
            load_non_writable<8>(__getAddress(address->traits)),
            load_non_writable<8>(__getAddress(address->triangles)),
            load_non_writable<8>(__getAddress(address->pieces)),
            load_non_writable<8>(__getAddress(address->body_from)),
            load_non_writable<8>(__getAddress(address->body_to)),
            load_non_writable<8>(__getAddress(address->body_triangles)),
            load_non_writable<8>(__getAddress(address->incident_starts)),
            load_non_writable<8>(__getAddress(address->incident)),
            load_non_writable<8>(__getAddress(address->epoch_motion)),
            load_non_writable<16>(__getAddress(address->counts)),
        };
    }
};

typealias Figures = NonWritableBuffer<Figure, 16>;

uint obstacle_figure(uint id) {
    return (id >> FIGURE_SHIFT) & FIGURE_MASK;
}

uint obstacle_index(uint id) {
    return id & INDEX_MASK;
}

bool obstacle_is_cloth(uint id) {
    return (id & CLOTH) != 0u;
}

uint grid_hash(int3 cell, uint level) {
    uint h = uint(cell.x) * 73856093u ^ uint(cell.y) * 19349663u ^ uint(cell.z) * 83492791u;
    return (h & (LEVEL_CELLS - 1u)) + level * LEVEL_CELLS;
}

float level_cell(uint level) {
    return level == 0u ? FINE_CELL : (level == 1u ? MIDDLE_CELL : COARSE_CELL);
}

int3 grid_cell(float3 x, uint level) {
    return int3(floor(x / level_cell(level)));
}

float3 face_normal(float3 a, float3 b, float3 c) {
    float3 n = cross(b - a, c - a);
    float length_squared = dot(n, n);
    return length_squared > 1.0e-24 ? n * rsqrt(length_squared) : float3(0.0);
}

struct Gap {
    float gap;
    float3 normal;
    float3 weights;
    bool valid;
};

Gap gap_to(float3 x, float3 a, float3 b, float3 c, uint entry) {
    Gap result = Gap(FOREVER, float3(0.0, 1.0, 0.0), float3(0.0), false);
    float3 n = face_normal(a, b, c);
    if (all(n == float3(0.0))) {
        return result;
    }
    Closest closest = closest_on_triangle(x, a, b, c);
    float d = dot(x - a, n);
    bool remembered = (entry & REMEMBERED) != 0u;
    float side = remembered ? ((entry & ABOVE) != 0u ? 1.0 : -1.0) : (d >= 0.0 ? 1.0 : -1.0);
    result.weights = closest.weights;
    result.valid = true;
    float3 away = x - closest.point;
    float lateral_squared = max(dot(away, away) - d * d, 0.0);
    bool beside = lateral_squared < LATERAL_REACH * LATERAL_REACH;
    if (closest.face || (remembered && side * d < 0.0 && beside)) {
        result.gap = side * d;
        result.normal = side * n;
    } else {
        float distance = length(away);
        result.gap = distance;
        result.normal = distance > 1.0e-12 ? away / distance : side * n;
    }
    return result;
}

struct Snug {
    bool in_range;
    bool above;
    bool valid;
    float apart;
    float rest_distance;
};

Snug snug_layer(ReadVec4s rest, uint4 corners, float3 at, float thick) {
    float4 ra = rest[corners.x];
    float4 rb = rest[corners.y];
    float4 rc = rest[corners.z];
    Closest closest = closest_on_triangle(at, ra.xyz, rb.xyz, rc.xyz);
    float apart = distance(at, closest.point);
    float3 normal = face_normal(ra.xyz, rb.xyz, rc.xyz);
    bool valid = any(normal != float3(0.0));
    bool above = dot(at - ra.xyz, normal) >= 0.0;
    float cover = above ? dot(closest.weights, float3(ra.w, rb.w, rc.w)) : 0.0;
    float clearance = apart - cover;
    return Snug(clearance < SNUG_LAYERS * thick, above, valid, clearance, apart);
}

float2 segment_params(float3 p1, float3 q1, float3 p2, float3 q2) {
    float3 d1 = q1 - p1;
    float3 d2 = q2 - p2;
    float3 r = p1 - p2;
    float a = dot(d1, d1);
    float e = dot(d2, d2);
    float f = dot(d2, r);
    if (a <= 1.0e-18 || e <= 1.0e-18) {
        return float2(-1.0);
    }
    float c = dot(d1, r);
    float b = dot(d1, d2);
    float denominator = a * e - b * b;
    float s = denominator > 1.0e-18 ? clamp((b * f - c * e) / denominator, 0.0, 1.0) : 0.0;
    float t = (b * s + f) / e;
    if (t < 0.0) {
        t = 0.0;
        s = clamp(-c / a, 0.0, 1.0);
    } else if (t > 1.0) {
        t = 1.0;
        s = clamp((b - c) / a, 0.0, 1.0);
    }
    return float2(s, t);
}

bool segment_crosses(float3 p, float3 q, float3 a, float3 b, float3 c) {
    float3 n = cross(b - a, c - a);
    float dp = dot(p - a, n);
    float dq = dot(q - a, n);
    if (dp * dq >= 0.0) {
        return false;
    }
    float t = dp / (dp - dq);
    float3 hit = p + (q - p) * t;
    float3 e0 = cross(b - a, hit - a);
    float3 e1 = cross(c - b, hit - b);
    float3 e2 = cross(a - c, hit - c);
    return dot(e0, n) > 0.0 && dot(e1, n) > 0.0 && dot(e2, n) > 0.0;
}

struct Collider : INonWritable {
    float4 start;
    float4 end;
    uint4 shape;

    static Collider load_from<let ALIGN : int>(Collider* address) {
        return {
            load_non_writable<16>(__getAddress(address->start)),
            load_non_writable<16>(__getAddress(address->end)),
            load_non_writable<16>(__getAddress(address->shape)),
        };
    }
};

typealias Colliders = NonWritableBuffer<Collider, 16>;

float2 collider_distance(Collider collider, ReadVec4s planes, float3 x, out float3 outward) {
    if (collider.shape.x == 0u) {
        float3 a = collider.start.xyz;
        float3 ab = collider.end.xyz - a;
        float length_squared = dot(ab, ab);
        float t = length_squared > 1.0e-18 ? clamp(dot(x - a, ab) / length_squared, 0.0, 1.0) : 0.0;
        float radius = lerp(collider.start.w, collider.end.w, t);
        float3 away = x - (a + ab * t);
        float distance = length(away);
        outward = distance > 1.0e-9 ? away / distance : float3(0.0, 1.0, 0.0);
        return float2(distance - radius, radius);
    }
    float deepest = -FOREVER;
    outward = float3(0.0, 1.0, 0.0);
    for (uint k = 0u; k < collider.shape.z; ++k) {
        float4 plane = planes[collider.shape.y + k];
        float d = dot(plane.xyz, x) - plane.w;
        if (d > deepest) {
            deepest = d;
            outward = plane.xyz;
        }
    }
    return float2(deepest, asfloat(collider.shape.w));
}

float2 collider_distance_past(Collider collider, ReadVec4s planes, float3 x, float enough) {
    if (collider.shape.x == 0u) {
        float3 outward;
        return collider_distance(collider, planes, x, outward);
    }
    float deepest = -FOREVER;
    for (uint k = 0u; k < collider.shape.z && deepest <= enough; ++k) {
        float4 plane = planes[collider.shape.y + k];
        deepest = max(deepest, dot(plane.xyz, x) - plane.w);
    }
    return float2(deepest, asfloat(collider.shape.w));
}

bool hull_pair_past(Collider collider, ReadVec4s planes, float3 p, float3 q, float enough) {
    float deepest_p = -FOREVER;
    float deepest_q = -FOREVER;
    for (uint k = 0u; k < collider.shape.z; ++k) {
        float4 plane = planes[collider.shape.y + k];
        deepest_p = max(deepest_p, dot(plane.xyz, p) - plane.w);
        deepest_q = max(deepest_q, dot(plane.xyz, q) - plane.w);
        if (deepest_p + deepest_q > enough) {
            return true;
        }
    }
    return false;
}

Collider collider_between(Colliders from, Colliders to, uint index, float alpha) {
    Collider a = from[index];
    Collider b = to[index];
    a.start = lerp(a.start, b.start, alpha);
    a.end = lerp(a.end, b.end, alpha);
    return a;
}

uint deepest_plane(Collider collider, ReadVec4s planes, float3 x) {
    float deepest = -FOREVER;
    uint chosen = 0u;
    for (uint k = 0u; k < collider.shape.z; ++k) {
        float4 plane = planes[collider.shape.y + k];
        float d = dot(plane.xyz, x) - plane.w;
        if (d > deepest) {
            deepest = d;
            chosen = k;
        }
    }
    return chosen;
}

float3 collider_exit(Collider now, Collider then, ReadVec4s planes_now, ReadVec4s planes_then, float3 x, float3 came_from, float radius) {
    if (now.shape.x != 0u) {
        float4 plane = planes_now[now.shape.y + deepest_plane(then, planes_then, came_from)];
        return plane.xyz * max(radius - (dot(plane.xyz, x) - plane.w), 0.0);
    }
    float3 a = now.start.xyz;
    float3 ab = now.end.xyz - a;
    float length_squared = dot(ab, ab);
    float t = length_squared > 1.0e-18 ? clamp(dot(x - a, ab) / length_squared, 0.0, 1.0) : 0.0;
    float3 away = x - (a + ab * t);
    float3 a0 = then.start.xyz;
    float3 ab0 = then.end.xyz - a0;
    float then_squared = dot(ab0, ab0);
    float t0 = then_squared > 1.0e-18 ? clamp(dot(came_from - a0, ab0) / then_squared, 0.0, 1.0) : 0.0;
    float3 came = came_from - (a0 + ab0 * t0);
    if (length_squared > 1.0e-18 && then_squared > 1.0e-18) {
        float3 axis = ab * rsqrt(length_squared);
        came = quat_rotate(quat_between(ab0 * rsqrt(then_squared), axis), came);
        came -= axis * dot(came, axis);
    }
    float came_length = length(came);
    float away_length = length(away);
    float r = lerp(now.start.w, now.end.w, t);
    bool came_deep = came_length < 0.5 * r && away_length > 1.0e-9 && dot(came, away) < 0.0;
    float3 outward = !came_deep && came_length > 1.0e-9 ? came / came_length
        : (away_length > 1.0e-9 ? away / away_length : float3(0.0, 1.0, 0.0));
    return outward * (r + radius) - away;
}
