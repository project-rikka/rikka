import rikka.effect;

struct Parameters {
    [Default(0.5, 0.5)] [Range(0.0, 1.0)] float2 center;
    [Default(0.5)] [Range(0.0, 1.0)] float inner;
    [Default(0.3)] [Range(0.0, 1.0)] float softness;
    [Default(150.0)] [Range(16.0, 600.0)] float density;
    [Default(0.55)] [Range(0.05, 1.0)] float thickness;
    [Default(0.7)] [Range(0.0, 1.0)] float roundness;
    [Default(0.0, 0.0, 0.0)] [Color] float3 color;
    [Default(0.85)] [Range(0.0, 1.0)] float opacity;
    [Default(12.0)] [Range(0.0, 60.0)] [Label("changes per second")] float rate;
    [Default(9)] uint seed;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
    float time;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float line_ink(Parameters parameters, float angle, float radius, float radius_pixels, uint seed) {
    float density = max(round(parameters.density), 1.0);
    float along = angle / RIKKA_TAU * density;
    float index = floor(along);
    uint3 key = uint3(uint(int(index)), seed, 0u);
    float centre = 0.5 + (rikka_effect_random(key) - 0.5) * 0.4;
    float width = parameters.thickness * lerp(0.15, 0.6, rikka_effect_random(key + uint3(0u, 0u, 1u)));
    float start = parameters.inner + lerp(0.0, 0.45, pow(rikka_effect_random(key + uint3(0u, 0u, 2u)), 1.5));
    if (rikka_effect_random(key + uint3(0u, 0u, 3u)) > 0.72) {
        return 0.0;
    }
    float taper = smoothstep(start, start + max(parameters.softness, 1e-3), radius);
    float half_width = 0.5 * width * taper;
    float pixel = density / max(RIKKA_TAU * radius_pixels, 1.0);
    float distance = abs(along - index - centre);
    return (1.0 - smoothstep(half_width - pixel, half_width + pixel, distance)) * taper;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void streak(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    float aspect = rikka_effect_aspect(push.size);
    float2 uv = rikka_effect_uv(pixel, push.size);
    float2 offset = (uv - parameters.center) * float2(lerp(1.0, aspect, parameters.roundness), 1.0);
    float2 pixels = (uv - parameters.center) * float2(push.size);
    float corner = length(float2(lerp(1.0, aspect, parameters.roundness), 1.0) * 0.5);
    float radius = length(offset) / corner;
    float angle = atan2(offset.y, offset.x) + RIKKA_PI;
    uint seed = parameters.seed * 7919u + uint(floor(max(push.time, 0.0) * parameters.rate));
    float ink = line_ink(parameters, angle, radius, length(pixels), seed);
    float matte = ink * clamp(parameters.opacity, 0.0, 1.0);
    float4 stroke = float4(rikka_effect_encode(parameters.color), 1.0);
    rikka_effect_store(push.target, pixel, lerp(color, stroke, matte));
}
