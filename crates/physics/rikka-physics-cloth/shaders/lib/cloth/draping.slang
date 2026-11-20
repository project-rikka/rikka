import cloth.common;
import cloth.world;
import cloth.bind;

static const float PUSH_REACH = 0.045;
static const uint PUSH_ROUNDS = 4u;
static const float LYING_FROM = 0.6;
static const float LYING_FULL = 0.9;
static const float PRESSED_FLAT = 0.7;

struct Placed {
    float3 position;
    float3 base;
    float3 normal;
    float3 tangent;
    uint3 home;
};

struct Planes<let N : int> {
    float4 at[N];
    uint count;

    __init() {
        count = 0u;
    }

    [mutating]
    void add(float4 plane) {
        at[count] = plane;
        count += 1u;
    }
}

float3 pushed_out<let N : int>(float3 p, Planes<N> planes) {
    for (uint round = 0u; round < PUSH_ROUNDS; ++round) {
        for (uint k = 0u; k < planes.count; ++k) {
            float short_by = planes.at[k].w - dot(planes.at[k].xyz, p);
            if (short_by > 0.0) {
                p += planes.at[k].xyz * short_by;
            }
        }
    }
    return p;
}

float3 unit_or(float3 v, float3 fallback) {
    float length_squared = dot(v, v);
    return length_squared > 1.0e-30 ? v * rsqrt(length_squared) : fallback;
}

struct Draping {
    ReadVec4s positions;
    ReadVec4s normals;
    ReadUvec4s triangles;
    float3 up;

    float3 particle(uint index) {
        return positions[index].xyz;
    }

    float relief(float3 normal, float rest_rise) {
        if (rest_rise >= LYING_FULL) {
            return 1.0;
        }
        float rise = dot(normal, up);
        return 1.0 - PRESSED_FLAT * smoothstep(max(rest_rise, LYING_FROM), LYING_FULL, rise);
    }

    Placed on_triangles(Bind bind) {
        Placed placed = Placed(float3(0.0), float3(0.0), float3(0.0), float3(0.0), triangles[bind.target[0].x].xyz);
        for (uint k = 0u; k < 3u; ++k) {
            uint4 place = bind.target[3u * k];
            float weight = asfloat(place.w);
            if (weight == 0.0) {
                continue;
            }
            float4 offset = asfloat(bind.target[3u * k + 1u]);
            float4 directions = asfloat(bind.target[3u * k + 2u]);
            uint4 corners = triangles[place.x];
            float3 a = particle(corners.x);
            float3 b = particle(corners.y);
            float3 c = particle(corners.z);
            float u = asfloat(place.y);
            float v = asfloat(place.z);
            float w = 1.0 - u - v;
            float3 face = unit_or(cross(b - a, c - a), float3(0.0, 1.0, 0.0));
            float3 blended = normals[corners.x].xyz * w + normals[corners.y].xyz * u
                + normals[corners.z].xyz * v;
            float3 normal = unit_or(blended, face);
            float3x3 frame = triangle_frame(a, b, c, normal);
            float kept = relief(normal, offset.w);
            float3 pressed = float3(1.0, 1.0, kept);
            float3 base = a * w + b * u + c * v;
            placed.base += weight * base;
            placed.position += weight * (base + mul(offset.xyz * pressed, frame));
            placed.normal += weight * mul(unpack_octahedral(directions.xy) * pressed.zzx, frame);
            placed.tangent += weight * mul(unpack_octahedral(directions.zw) * pressed, frame);
        }
        return placed;
    }
}
