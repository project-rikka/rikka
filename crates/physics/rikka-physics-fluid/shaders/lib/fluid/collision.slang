import fluid.settings;
import fluid.walls;
import fluid.slab;
import fluid.listed;
import physics.reaction;
import physics.triangle;

struct BodyTable : INonWritable {
    ReadVec4s faces;
    ReadVec4s spheres;
    ReadUvec4s triangles;
    ReadUints tri_starts;
    ReadUints tri_items;
    Uints wet;
    ReactionWords reactions;

    static BodyTable load_from<let ALIGN : int>(BodyTable* address) {
        return {
            load_non_writable<8>(__getAddress(address->faces)),
            load_non_writable<8>(__getAddress(address->spheres)),
            load_non_writable<8>(__getAddress(address->triangles)),
            load_non_writable<8>(__getAddress(address->tri_starts)),
            load_non_writable<8>(__getAddress(address->tri_items)),
            load_non_writable<8>(__getAddress(address->wet)),
            load_non_writable<8>(__getAddress(address->reactions)),
        };
    }
};

typealias BodyTableRef = NonWritableReference<BodyTable, 8>;
static const uint REMEMBERED = 0x80000000u;
static const uint ABOVE = 0x40000000u;
static const uint TRIANGLE = 0x3fffffffu;
static const uint ROUNDS = 3u;

struct Touch {
    uint triangle;
    float gap;
    float3 normal;
    float3 weights;
};

static const Touch UNTOUCHED = Touch(NONE, FOREVER, float3(0.0, 1.0, 0.0), float3(0.0));

Touch touching(ReadVec4s faces, ReadVec4s spheres, uint entry, float3 x, float beaten) {
    uint t = entry & TRIANGLE;
    float4 plane = spheres[2u * t + 1u];
    Touch touch = UNTOUCHED;
    touch.triangle = t;
    if (all(plane.xyz == float3(0.0))) {
        return touch;
    }
    float d1 = dot(x, plane.xyz) - plane.w;
    bool remembered = (entry & REMEMBERED) != 0u;
    float side = remembered ? ((entry & ABOVE) != 0u ? 1.0 : -1.0) : (d1 >= 0.0 ? 1.0 : -1.0);
    float signed1 = side * d1;
    if (signed1 >= beaten) {
        return touch;
    }
    uint at = t * FACE_VECS;
    float3 a1 = faces[at].xyz;
    float3 b1 = faces[at + 1u].xyz;
    float3 c1 = faces[at + 2u].xyz;
    float3 n1 = plane.xyz;
    Closest closest = closest_on_triangle(x, a1, b1, c1);
    float3 w1 = closest.weights;
    float3 q1 = w1.x * a1 + w1.y * b1 + w1.z * c1;
    float dist1 = length(x - q1);
    touch.weights = w1;
    if (closest.face || (remembered && signed1 < 0.0)) {
        touch.normal = side * n1;
        touch.gap = signed1;
    } else {
        touch.gap = dist1;
        touch.normal = dist1 > 0.0 ? (x - q1) / dist1 : side * n1;
    }
    return touch;
}

float3 travel_of(ReadVec4s faces, Touch touch) {
    uint at = touch.triangle * FACE_VECS;
    float3 w = touch.weights;
    return w.x * faces[at + 3u].xyz + w.y * faces[at + 4u].xyz + w.z * faces[at + 5u].xyz;
}

struct Pushed {
    float3 x;
    Touch support;
    Touch nearest;
    float4 plane;
    float grounded;
    float clearance;
};

void nearer_plane(inout Pushed pushed, float4 plane, float3 x) {
    if (dot(plane.xyz, x) - plane.w < dot(pushed.plane.xyz, x) - pushed.plane.w) {
        pushed.plane = plane;
    }
}

Pushed push_out(Settings s, ReadVec4s faces, ReadVec4s spheres, Uints cache, uint stride, uint i, float3 x, float3 x0, float mu) {
    float thickness = s.body.x;
    float band = s.body.y;
    Pushed pushed = Pushed(x, UNTOUCHED, UNTOUCHED, float4(0.0, 1.0, 0.0, -FOREVER), 0.0, FOREVER);
    float4 planes[ROUNDS + 1u];
    for (uint k = 0u; k <= ROUNDS; ++k) {
        planes[k] = pushed.plane;
    }
    if (s.body.w > 0.5) {
        float ground = s.body.z + s.bounds_lo.w;
        if (x.y < ground) {
            x = landed(x0, x, ground, mu);
        }
        pushed.grounded = 1.0 - clamp((x.y - ground) / band, 0.0, 1.0);
        planes[0] = float4(0.0, 1.0, 0.0, ground - thickness);
        pushed.clearance = x.y - ground + thickness;
    }
    if (s.tri_grid.w > 0u) {
        uint entries[MAX_CANDIDATES];
        uint candidate_count = 0u;
        [ForceUnroll]
        for (uint k = 0u; k < MAX_CANDIDATES; ++k) {
            entries[k] = cache[k * stride + i];
            if (entries[k] == NONE) {
                break;
            }
            candidate_count += 1u;
        }
        for (uint round = 0u; round < ROUNDS; ++round) {
            Touch best = UNTOUCHED;
            [ForceUnroll]
            for (uint k = 0u; k < MAX_CANDIDATES; ++k) {
                if (k >= candidate_count) {
                    break;
                }
                Touch touch = touching(faces, spheres, entries[k], x, best.gap);
                if (touch.gap < best.gap) {
                    best = touch;
                }
            }
            if (round == 0u) {
                pushed.clearance = min(pushed.clearance, best.gap);
                pushed.nearest = best;
            }
            if (best.triangle == NONE || best.gap >= thickness + band) {
                break;
            }
            planes[1u + round] = float4(best.normal, dot(best.normal, x) - best.gap);
            if (round == 0u) {
                pushed.support = best;
            }
            float depth = thickness - best.gap;
            if (depth <= 0.0) {
                break;
            }
            x += best.normal * depth;
            if (round == 0u) {
                pushed.support.gap = thickness;
                float3 moved = (x - x0) - travel_of(faces, best);
                float3 tangential = moved - best.normal * dot(moved, best.normal);
                float slide = length(tangential);
                if (slide > 0.0) {
                    x -= tangential * min(mu * depth / slide, 1.0);
                }
            }
        }
    }
    if (any(x != pushed.x)) {
        pushed.clearance = 0.0;
    }
    pushed.x = x;
    for (uint k = 0u; k <= ROUNDS; ++k) {
        nearer_plane(pushed, planes[k], x);
    }
    return pushed;
}

uint2 pack_plane(float4 plane, float3 x) {
    return pack_wall(float4(plane.xyz, dot(plane.xyz, x) - plane.w), x);
}

uint4 anchored_at(Touch touch, float3 normal, float rest) {
    uint above = dot(normal, touch.normal) >= 0.0 ? ABOVE : 0u;
    return uint4(touch.triangle | above, asuint(touch.weights.y), asuint(touch.weights.z), asuint(rest));
}

float3 anchor_point(BodyTable bodies, uint4 anchor) {
    uint t = anchor.x & TRIANGLE;
    uint at = t * FACE_VECS;
    float3 a = bodies.faces[at].xyz;
    float3 b = bodies.faces[at + 1u].xyz;
    float3 c = bodies.faces[at + 2u].xyz;
    float3 surface = a + asfloat(anchor.y) * (b - a) + asfloat(anchor.z) * (c - a);
    float side = (anchor.x & ABOVE) != 0u ? 1.0 : -1.0;
    return surface + side * asfloat(anchor.w) * bodies.spheres[2u * t + 1u].xyz;
}

float3 pull_to_anchor(BodyTable bodies, inout uint4 anchor, float3 x, Substance kind) {
    if (anchor.x == NONE) {
        return x;
    }
    float3 pull = anchor_point(bodies, anchor) - x;
    if (dot(pull, pull) > kind.contact.w * kind.contact.w) {
        anchor = uint4(NONE);
        return x;
    }
    return x + pull * kind.stick.y;
}
