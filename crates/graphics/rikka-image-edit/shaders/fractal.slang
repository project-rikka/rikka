import plane;

struct Push {
    Head head;
    float scale;
    uint detail;
    uint seed;
    float evolution;
    float contrast;
    float brightness;
    float2 offset;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

float3 gradient(int3 cell, uint octave) {
    uint3 at = uint3(cell) ^ uint3(0u, 0u, pc.seed * 0x9e3779b9u + octave * 0x85ebca6bu);
    float theta = hash01(at) * 6.28318530718;
    float z = hash01(at.zxy ^ uint3(0x68e31da4u)) * 2.0 - 1.0;
    float r = sqrt(max(1.0 - z * z, 0.0));
    return float3(r * cos(theta), r * sin(theta), z);
}

float3 fade(float3 t) {
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
}

float perlin(float3 p, uint octave) {
    int3 cell = int3(floor(p));
    float3 f = p - float3(cell);
    float3 u = fade(f);
    float corners[8];
    for (int i = 0; i < 8; i++) {
        int3 corner = int3(i & 1, (i >> 1) & 1, (i >> 2) & 1);
        corners[i] = dot(gradient(cell + corner, octave), f - float3(corner));
    }
    float x00 = lerp(corners[0], corners[1], u.x);
    float x10 = lerp(corners[2], corners[3], u.x);
    float x01 = lerp(corners[4], corners[5], u.x);
    float x11 = lerp(corners[6], corners[7], u.x);
    return lerp(lerp(x00, x10, u.y), lerp(x01, x11, u.y), u.z);
}

static const uint COLORED = 1u;

float fractal(float2 at, uint layer) {
    float sum = 0.0;
    float weight = 0.0;
    float amplitude = 1.0;
    float frequency = 1.0;
    for (uint octave = 0u; octave < pc.detail; octave++) {
        float3 p = float3(at * frequency, pc.evolution * frequency) + float3(float(octave) * 17.31);
        sum += amplitude * perlin(p, octave + layer * 64u);
        weight += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    float value = 0.5 + 0.75 * sum / max(weight, 1e-6);
    return clamp((value - 0.5) * pc.contrast + 0.5 + pc.brightness, 0.0, 1.0);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void fractal_noise(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float2 at = (float2(pixel) + 0.5 + pc.offset) / pc.scale;
    float3 color;
    if (pc.head.mode == COLORED) {
        color = float3(fractal(at, 0u), fractal(at, 1u), fractal(at, 2u));
    } else {
        color = float3(fractal(at, 0u));
    }
    pc.head.dst.store(pixel, pc.head.clamped(float4(color, 1.0)));
}
