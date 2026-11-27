__exported import cloth.common;
__exported import cloth.world;
__exported import cloth.job;

static const uint THREADS = 256u;
static const uint PLANNED = 256u;
static const float AIR_DENSITY = 1.2;
static const float WIND_SCALE = 0.35;
static const float WIND_DRIFT = 0.6;
static const float TOUCH_FADE = 0.3;
static const float FLUTTER_WIND = 3.0;
static const float PUSH_SPEED = 3.0;
static const float RESCUE_STEP = 0.008;
static const float DAT_SHARE = 0.425;
static const float SAME_SPAN = 1.0e-5;
static const float EXTENT_SHARE = 0.1;
static const float EXTENT_HELD = 0.15;
static const float TEARING = 1.2;
static const float HARD_TEARING = 1.8;
static const float LIMB_CORE = 0.35;
static const uint LIMB_SAMPLES = 5u;
static const float VERDICT_MARGIN = 0.15;
static const float WRAP_SHARE = 0.4;
static const float PINCH_FACING = -0.3;
static const float WRAP_RIM = 0.12;
static const float TOUCH_SCALE = 0.98;
static const float TOUCH_LOCK = 0.5;
static const float BARRIER_SETTLE = 0.85;
static const float SELF_SETTLE = 0.80;
static const float POST_STEP = 0.015;
static const float FACING_BLEND = 0.25;
static const float SNUG_MIN = 0.0005;
static const float POST_MESH_MIN = 0.0005;
static const float POST_MESH_MAX = 0.04;
static const float OUTSIDE_COLLIDER_SHARE = 0.08;
static const float OUTSIDE_COLLIDER_DEPTH = 0.004;
static const float2 SHOCK_SPEED = float2(0.05, 0.35);
static const float2 SHOCK_OPPOSE = float2(0.35, 0.85);
static const float SHOCK_DAMP = 0.70;
static const float AXIAL_DAMP = 0.20;
static const float BEND_RAYLEIGH = 0.006;
static const float2 MICRO_PACE = float2(0.002, 0.010);
static const float MICRO_DAMP = 0.55;
static const float OVERLAP_DEPTH = 0.001;
static const float DRIFT_MEMORY = 0.85;
static const float2 TREMBLING = float2(0.45, 0.80);
static const uint STATS_WORDS = 3u;
static const float STIRRED_SPEED = 0.1;
static const float REACH_SHARE = 0.5;
static const float RESTORE_GAIN = 20.0;

struct Push {
    Jobs jobs;
    uint first_job;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

static Job job = {};
static uint first = 0u;
static uint count = 0u;
static uint lane = 0u;
static uint planned = 0u;

groupshared uint fastest;
groupshared uint jittering;
groupshared uint moving;
groupshared uint ranges[PLANNED];

uint plan(uint index) {
    uint at = index - job.colours.x;
    return at < planned ? shared_load(__getAddress(ranges[at])) : job.plan[index];
}

bool mine(uint i) {
    return i - first < count;
}
float3 target(uint i, float alpha) {
    float3 from = asfloat(job.targets_from[i].xyz);
    float3 to = asfloat(job.targets_to[i].xyz);
    return lerp(from, to, alpha);
}

float3 target_normal(uint i, float alpha) {
    float3 from = unpack_direction(job.targets_from[i].w);
    float3 to = unpack_direction(job.targets_to[i].w);
    float3 mixed = lerp(from, to, alpha);
    float length_squared = dot(mixed, mixed);
    return length_squared > 1.0e-12 ? mixed * rsqrt(length_squared) : from;
}

void cached_body(uint i, uint k, float4 g4, out float3 a0, out float3 b0, out float3 c0, out float3 a1, out float3 b1, out float3 c1) {
    uint at = (i * BODY_CANDIDATES + k) * GEOMETRY_VECS;
    float4 g0 = job.geometry[at];
    float4 g1 = job.geometry[at + 1u];
    float4 g2 = job.geometry[at + 2u];
    float4 g3 = job.geometry[at + 3u];
    a0 = g0.xyz;
    b0 = float3(g0.w, g1.xy);
    c0 = float3(g1.zw, g2.x);
    a1 = g2.yzw;
    b1 = g3.xyz;
    c1 = float3(g3.w, g4.xy);
}

void cached_body_end(uint i, uint k, float4 g4, out float3 a1, out float3 b1, out float3 c1) {
    uint at = (i * BODY_CANDIDATES + k) * GEOMETRY_VECS;
    float4 g2 = job.geometry[at + 2u];
    float4 g3 = job.geometry[at + 3u];
    a1 = g2.yzw;
    b1 = g3.xyz;
    c1 = float3(g3.w, g4.xy);
}

float collider_limit(bool outside, float thickness) {
    return outside ? min(-OUTSIDE_COLLIDER_SHARE * thickness, -OUTSIDE_COLLIDER_DEPTH) : 0.0;
}

float pressed_code(float3 normal, float touch) {
    uint2 q = uint2(round(clamp(octahedral(normal) * 0.5 + 0.5, 0.0, 1.0) * 255.0));
    return float(1u + q.x + 256u * q.y) + clamp(touch, 0.0, 1.0) * TOUCH_SCALE;
}

float3 pressed_normal(float code) {
    uint packed = uint(code) - 1u;
    return unpack_octahedral(float2(packed & 255u, packed >> 8u) * (2.0 / 255.0) - 1.0);
}

float pressed_touch(float code) {
    return code > 0.0 ? frac(code) / TOUCH_SCALE : 0.0;
}

float3 clamp_outward(float3 delta, float code) {
    if (!(code > 0.0)) {
        return delta;
    }
    float3 normal = pressed_normal(code);
    return delta - normal * (min(dot(delta, normal), 0.0) * pressed_touch(code));
}

void slot(uint at, float3 delta) {
    if (at != NONE) {
        job.slots[at] = float4(delta, 0.0);
    }
}

void halt(uint i, float3 toward) {
    float4 v = job.velocities[i];
    float along = dot(v.xyz, toward);
    if (along < 0.0) {
        job.velocities[i] = float4(v.xyz - toward * along, v.w);
    }
}

float3 undoing(float3 corrected, float3 normal, float push) {
    return normal * clamp(-dot(corrected, normal), 0.0, max(push, 0.0));
}

