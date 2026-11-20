import cloth.common;
import cloth.world;
import cloth.bind;
import cloth.draping;

static const float LAYER_GAP = 0.001;
static const float LAYER_CONE = 0.7;
static const float REST_NEAR = 0.03;
static const uint LAYER_TRIANGLES = 64u;
static const uint PUSH_PLANES = LAYER_TRIANGLES;
static const uint LIFT_WORD = 3u;

struct Push {
    ReadVec4s positions;
    ReadVec4s normals;
    ReadUvec4s triangles;
    Binds binds;
    ReadUints nearest;
    ReadVec4s rest;
    ReadFloats reliefs;
    ReadUints incident_starts;
    ReadUints incident;
    Uints lifts;
    uint count;
    float up_x;
    float up_y;
    float up_z;
};
[[vk::push_constant]] ConstantBuffer<Push> pc;

void layer_plane(Draping draping, inout Planes<PUSH_PLANES> planes, float3 p, float3 drawn_rest, uint triangle) {
    uint4 corners = pc.triangles[triangle];
    float3 ra = pc.rest[corners.x].xyz;
    float3 rb = pc.rest[corners.y].xyz;
    float3 rc = pc.rest[corners.z].xyz;
    float3 rest_normal = face_normal(ra, rb, rc);
    float rest_height = dot(drawn_rest - ra, rest_normal);
    float side = rest_height >= 0.0 ? 1.0 : -1.0;
    float3 a = draping.particle(corners.x);
    float3 b = draping.particle(corners.y);
    float3 c = draping.particle(corners.z);
    float3 n = side * face_normal(a, b, c);
    if (all(rest_normal == float3(0.0)) || all(n == float3(0.0))) {
        return;
    }
    uint lifted = side > 0.0 ? 0u : 1u;
    float relief = max(max(pc.reliefs[2u * corners.x + lifted], pc.reliefs[2u * corners.y + lifted]),
        pc.reliefs[2u * corners.z + lifted]);
    float ceiling = relief + LAYER_GAP;
    float height = dot(p - a, n);
    if (height >= ceiling || height <= -PUSH_REACH) {
        return;
    }
    float required = ceiling;
    float abs_rest = abs(rest_height);
    if (abs_rest < min(ceiling, REST_NEAR)) {
        Closest rest_closest = closest_on_triangle(drawn_rest, ra, rb, rc);
        if (distance(drawn_rest, rest_closest.point) < REST_NEAR) {
            required = abs_rest;
            if (height >= required) {
                return;
            }
        }
    }
    Closest closest = closest_on_triangle(p, a, b, c);
    float3 away = p - closest.point;
    float lateral_sq = max(dot(away, away) - height * height, 0.0);
    bool behind_edge = height < 0.0 && lateral_sq < LATERAL_REACH * LATERAL_REACH;
    if (!closest.face && !behind_edge && dot(away, n) < LAYER_CONE * length(away)) {
        return;
    }
    planes.add(float4(n, dot(n, a) + required));
}

struct Layered {
    uint seen[LAYER_TRIANGLES];
    uint count;

    __init() {
        seen = {};
        count = 0u;
    }

    [mutating]
    void once(uint triangle) {
        if (count >= LAYER_TRIANGLES) {
            return;
        }
        for (uint k = 0u; k < count; ++k) {
            if (seen[k] == triangle) {
                return;
            }
        }
        seen[count] = triangle;
        count += 1u;
    }

    [mutating]
    void ring(uint triangle) {
        uint4 corners = pc.triangles[triangle];
        uint ids[3] = { corners.x, corners.y, corners.z };
        for (uint m = 0u; m < 3u && count < LAYER_TRIANGLES; ++m) {
            uint to = pc.incident_starts[ids[m] + 1u];
            for (uint at = pc.incident_starts[ids[m]]; at < to && count < LAYER_TRIANGLES; ++at) {
                once(pc.incident[at]);
            }
        }
    }
}

Layered layered_under(Bind bind) {
    Layered layered = Layered();
    uint underlay = bind.target[DRAWN_REST].w;
    if (underlay != NONE) {
        layered.ring(underlay);
    }
    for (uint influence = 0u; influence < 3u; ++influence) {
        uint4 place = bind.target[3u * influence];
        if (influence > 0u && asfloat(place.w) == 0.0) {
            continue;
        }
        uint4 home = pc.triangles[place.x];
        uint particles[3] = { home.x, home.y, home.z };
        for (uint corner = 0u; corner < 3u; ++corner) {
            for (uint slot = 0u; slot < LAYER_NEAREST; ++slot) {
                uint entry = pc.nearest[NEAREST_WORDS * particles[corner] + BODY_NEAREST + slot];
                if (entry == NONE) {
                    break;
                }
                uint nearby = obstacle_index(entry);
                if (influence == 0u && slot == 0u) {
                    layered.ring(nearby);
                } else {
                    layered.once(nearby);
                }
            }
        }
    }
    return layered;
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint at = thread.x;
    if (at >= pc.count) {
        return;
    }
    Bind bind = pc.binds[at];
    if ((bind.header.x & STRAND) != 0u) {
        return;
    }
    Layered layered = layered_under(bind);
    if (layered.count == 0u) {
        return;
    }
    Draping draping = { pc.positions, pc.normals, pc.triangles, float3(pc.up_x, pc.up_y, pc.up_z) };
    float3 position = draping.on_triangles(bind).position;
    float3 drawn_rest = asfloat(bind.target[DRAWN_REST].xyz);
    Planes<PUSH_PLANES> planes = Planes<PUSH_PLANES>();
    for (uint k = 0u; k < layered.count; ++k) {
        layer_plane(draping, planes, position, drawn_rest, layered.seen[k]);
    }
    float short_by = distance(pushed_out(position, planes), position);
    if (!(short_by > 0.0)) {
        return;
    }
    for (uint k = 0u; k < 3u; ++k) {
        uint4 place = bind.target[3u * k];
        if (asfloat(place.w) == 0.0) {
            continue;
        }
        uint4 corners = pc.triangles[place.x];
        queue_atomic_max(pc.lifts.at(4u * corners.x + LIFT_WORD), asuint(short_by));
        queue_atomic_max(pc.lifts.at(4u * corners.y + LIFT_WORD), asuint(short_by));
        queue_atomic_max(pc.lifts.at(4u * corners.z + LIFT_WORD), asuint(short_by));
    }
}
