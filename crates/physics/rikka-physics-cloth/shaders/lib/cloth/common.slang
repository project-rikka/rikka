__exported import physics.memory;
__exported import physics.matrix;
__exported import physics.octahedral;

typealias Exchanged4s = Exchanged<float4, 16>;
typealias ExchangedUvec4s = Exchanged<uint4, 16>;
typealias ExchangedUints = Exchanged<uint, 4>;

void sync() {
    exchange_barrier();
}
static const uint NONE = 0xffffffffu;
static const float PI = 3.14159265358979;
static const float FRAME_RATE = 60.0;
static const uint SURFACE = 1u;

float3 quat_rotate(float4 q, float3 v) {
    float3 t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

float4 quat_between(float3 from, float3 to) {
    float d = dot(from, to);
    if (d < -0.999999) {
        float3 axis = cross(float3(1.0, 0.0, 0.0), from);
        if (dot(axis, axis) < 1.0e-6) {
            axis = cross(float3(0.0, 1.0, 0.0), from);
        }
        return float4(normalize(axis), 0.0);
    }
    return normalize(float4(cross(from, to), 1.0 + d));
}

float3 rotate_about(float3 v, float3 axis, float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return v * c + cross(axis, v) * s + axis * dot(axis, v) * (1.0 - c);
}

float damped(float per_frame, float h) {
    return pow(max(1.0 - per_frame, 0.0), h * FRAME_RATE);
}

float per_substep(float per_frame, float h) {
    return 1.0 - damped(clamp(per_frame, 0.0, 1.0), h);
}

uint hash(uint3 cell, uint seed) {
    uint h = cell.x * 0x8da6b343u ^ cell.y * 0xd8163841u ^ cell.z * 0xcb1ab31fu ^ seed;
    h ^= h >> 16;
    h *= 0x7feb352du;
    h ^= h >> 15;
    h *= 0x846ca68bu;
    h ^= h >> 16;
    return h;
}

float lattice(int3 cell, uint seed) {
    return float(hash(uint3(cell), seed) & 0xffffu) / 32767.5 - 1.0;
}

float value_noise(float3 p, uint seed) {
    float3 cell = floor(p);
    float3 f = p - cell;
    float3 u = f * f * (3.0 - 2.0 * f);
    int3 c = int3(cell);
    float x00 = lerp(lattice(c, seed), lattice(c + int3(1, 0, 0), seed), u.x);
    float x10 = lerp(lattice(c + int3(0, 1, 0), seed), lattice(c + int3(1, 1, 0), seed), u.x);
    float x01 = lerp(lattice(c + int3(0, 0, 1), seed), lattice(c + int3(1, 0, 1), seed), u.x);
    float x11 = lerp(lattice(c + int3(0, 1, 1), seed), lattice(c + int3(1, 1, 1), seed), u.x);
    return lerp(lerp(x00, x10, u.y), lerp(x01, x11, u.y), u.z);
}

float3 vector_noise(float3 p) {
    return float3(
        value_noise(p, 0x2545f491u),
        value_noise(p + float3(31.416, -47.853, 12.793), 0x9e3779b9u),
        value_noise(p + float3(-19.172, 23.337, 71.917), 0x85ebca6bu));
}

float3 any_orthonormal(float3 n) {
    float sign_z = n.z >= 0.0 ? 1.0 : -1.0;
    float a = -1.0 / (sign_z + n.z);
    float b = n.x * n.y * a;
    return float3(b, sign_z + n.y * n.y * a, -n.y);
}

float3x3 triangle_frame(float3 a, float3 b, float3 c, float3 normal) {
    float3 edge = b - a;
    float3 tangent = edge - normal * dot(normal, edge);
    float length_squared = dot(tangent, tangent);
    if (!(length_squared > 0.0)) {
        float3 other = c - a;
        tangent = other - normal * dot(normal, other);
        length_squared = dot(tangent, tangent);
    }
    tangent = length_squared > 0.0 ? tangent * rsqrt(length_squared) : any_orthonormal(normal);
    return float3x3(tangent, cross(normal, tangent), normal);
}

float3 clamp_length(float3 v, float cap) {
    float l = length(v);
    return l > cap && l > 1.0e-9 ? v * (cap / l) : v;
}
