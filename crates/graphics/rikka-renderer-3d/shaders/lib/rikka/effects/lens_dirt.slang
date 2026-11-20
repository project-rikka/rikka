__exported import rikka.effects.blur;

struct Procedural {
    [Default(1.0)] [Range(0.25, 4.0)] float scale;
    [Default(0.3)] [Range(0.0, 1.0)] float density;
    [Default(0.6)] [Range(0.0, 1.0)] float smudge;
    [Default(7)] uint seed;
};

struct Parameters {
    [Texture("colour")] uint dirt;
    [Default(1.2)] [Range(0.0, 8.0)] float intensity;
    [Default(1.0, 1.0, 1.0)] [Color] float3 tint;
    [Default(0.5)] [Range(-6.0, 10.0)] [Group("glare")] [Label("threshold (EV over middle grey)")] float threshold;
    [Default(1.0)] [Range(0.0, 4.0)] [Group("glare")] [Label("knee (EV)")] float knee;
    [Default(60.0)] [Range(4.0, 160.0)] [Group("glare")] float radius;
    [Group("procedural")] Procedural procedural;
};

static const float RIKKA_DIRT_CELLS = 5.0;
static const int RIKKA_DIRT_LAYERS = 3;
static const float RIKKA_DIRT_FIREFLY = 64.0;

float rikka_dirt_spots(float2 position, Procedural procedural, int layer) {
    float2 grid = position * (1.0 + 0.85 * float(layer));
    int2 cell = int2(floor(grid));
    float2 local = grid - float2(cell);
    uint3 seed = uint3(uint(cell.x), uint(cell.y), procedural.seed * 31u + uint(layer));
    if (rikka_effect_random(seed) > procedural.density) {
        return 0.0;
    }
    float radius = lerp(0.08, 0.34, rikka_effect_random(seed + uint3(0u, 0u, 7u)));
    float2 centre = float2(
        lerp(radius, 1.0 - radius, rikka_effect_random(seed + uint3(0u, 0u, 13u))),
        lerp(radius, 1.0 - radius, rikka_effect_random(seed + uint3(0u, 0u, 19u))));
    float2 offset = (local - centre) / radius;
    float wobble = 1.0 + 0.25 * rikka_effect_noise(float3(offset * 1.7, float(layer) * 3.1), procedural.seed + 3u);
    float distance = length(offset) * wobble;
    float disc = 1.0 - smoothstep(0.15, 1.0, distance);
    float rim = smoothstep(0.7, 0.92, distance) * (1.0 - smoothstep(0.92, 1.0, distance));
    float strength = lerp(0.2, 0.8, rikka_effect_random(seed + uint3(0u, 0u, 23u)));
    return (disc * 0.6 + rim * 0.25) * strength / (1.0 + 0.6 * float(layer));
}

float3 rikka_dirt_procedural(Procedural procedural, float2 uv, float aspect) {
    float2 position = float2(uv.x * aspect, uv.y) * RIKKA_DIRT_CELLS / max(procedural.scale, 0.05);
    float smudge = rikka_effect_fbm(float3(position * 0.45, 0.5), procedural.seed, 4) * 0.5 + 0.5;
    float amount = smoothstep(0.45, 0.85, smudge) * procedural.smudge;
    for (int layer = 0; layer < RIKKA_DIRT_LAYERS; ++layer) {
        amount += rikka_dirt_spots(position, procedural, layer);
    }
    float streak = rikka_effect_fbm(float3(position.x * 2.5, position.y * 0.25, 3.0), procedural.seed + 5u, 3);
    amount += smoothstep(0.35, 0.7, streak) * 0.25 * procedural.smudge;
    return float3(amount);
}

float3 rikka_dirt(Parameters parameters, float2 uv, float aspect) {
    if (parameters.dirt != RIKKA_NONE) {
        return rikka_effect_sample_repeat(parameters.dirt, uv).rgb;
    }
    return rikka_dirt_procedural(parameters.procedural, uv, aspect);
}

float4 rikka_dirt_extract(Parameters parameters, uint source, int2 pixel, uint2 size) {
    float4 reduced = rikka_blur_reduce(source, pixel, size);
    float luma = rikka_luma(max(reduced.rgb, float3(0.0)));
    float kept = rikka_effect_bright(luma, parameters.threshold, parameters.knee);
    float3 glare = max(reduced.rgb, float3(0.0)) * kept;
    glare *= min(1.0, RIKKA_DIRT_FIREFLY / max(rikka_luma(glare), 1e-6));
    return float4(glare, 1.0);
}

extension Parameters : IBlurParameters {
    static float sigma(Parameters* parameters, uint2 size) {
        Parameters values = *parameters;
        return 0.5 * values.radius * rikka_effect_scale(size);
    }
}
