import rikka.effect;

struct Strokes {
    [Default(6.0)] [Range(2.0, 32.0)] [Label("spacing (px at 1080p)")] float spacing;
    [Default(0.32)] [Range(0.05, 0.9)] float width;
    [Default(0.35)] [Range(0.0, 2.0)] float wobble;
    [Default(0.35)] [Range(0.0, 1.0)] float breaks;
    [Default(0.0)] [Range(0.0, 1.0)] float angle;
};

struct Edges {
    [Default(1.0)] [Range(0.0, 2.0)] float strength;
    [Default(0.08)] [Range(0.0, 1.0)] float threshold;
    [Default(0.12)] [Range(0.0, 1.0)] [Label("depth threshold")] float depth;
};

struct Parameters {
    [Group("strokes")] Strokes strokes;
    [Group("edges")] Edges edges;
    [Default(0.9)] [Range(0.0, 1.0)] float darkness;
    [Default(0.85, 0.82, 0.75)] [Color] [Group("colours")] float3 paper;
    [Default(0.012, 0.012, 0.018)] [Color] [Group("colours")] float3 pencil;
    [Default(0.2)] [Range(0.0, 1.0)] [Group("colours")] float color;
    [Default(0.0)] [Range(0.0, 24.0)] [Label("boil rate (per second)")] float rate;
    [Default(5)] uint seed;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint depth;
    uint target;
    uint2 size;
    RikkaFrameRef frame;
    float time;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int LAYERS = 4;
static const float ANGLES[4] = { 0.62, -0.62, 1.42, 0.12 };
static const float THRESHOLDS[4] = { 0.7, 0.5, 0.32, 0.16 };

float tone_at(int2 pixel) {
    return rikka_luma(clamp(rikka_effect_straight(rikka_effect_load_clamped(push.source, pixel, push.size)), 0.0, 1.0));
}

float depth_at(int2 pixel) {
    return log2(rikka_effect_linear_depth(push.frame, rikka_effect_load_clamped(push.depth, pixel, push.size).r));
}

float edge_ink(Edges edges, int2 pixel, int reach) {
    float l = tone_at(pixel + int2(-reach, 0));
    float r = tone_at(pixel + int2(reach, 0));
    float u = tone_at(pixel + int2(0, -reach));
    float d = tone_at(pixel + int2(0, reach));
    float luma = length(float2(r - l, d - u));
    float dl = depth_at(pixel + int2(-reach, 0));
    float dr = depth_at(pixel + int2(reach, 0));
    float du = depth_at(pixel + int2(0, -reach));
    float dd = depth_at(pixel + int2(0, reach));
    float depth = max(abs(dr - dl), abs(dd - du));
    float from_luma = smoothstep(edges.threshold, edges.threshold * 2.0 + 0.02, luma);
    float from_depth = smoothstep(edges.depth, edges.depth * 2.0 + 0.02, depth);
    return clamp(max(from_luma, from_depth) * edges.strength, 0.0, 1.0);
}

float hatching(Strokes strokes, float2 position, float spacing, float tone, uint seed) {
    float ink = 0.0;
    float smoothing = 0.75 / spacing;
    for (int layer = 0; layer < LAYERS; ++layer) {
        float weight = clamp((THRESHOLDS[layer] - tone) / 0.16, 0.0, 1.0);
        if (weight <= 0.0) {
            continue;
        }
        float2 rotated = rikka_effect_rotate(position, -(ANGLES[layer] + strokes.angle * RIKKA_PI));
        float along = rotated.x / spacing;
        float across = rotated.y / spacing;
        float bend = rikka_effect_noise(float3(along * 0.08, across * 0.08, float(layer)), seed) * strokes.wobble;
        float shifted = across + bend;
        float row = floor(shifted);
        float offset = abs(shifted - row - 0.5);
        float half_width = 0.5 * strokes.width * lerp(0.7, 1.15, rikka_effect_random(uint3(uint(int(row)), uint(layer), seed)));
        float stroke = 1.0 - smoothstep(half_width - smoothing, half_width + smoothing, offset);
        float gap = rikka_effect_noise(float3(along * 0.12 + row * 7.31, row, float(layer) + 9.0), seed + 17u);
        stroke *= smoothstep(strokes.breaks - 0.15, strokes.breaks + 0.15, gap * 0.5 + 0.5);
        ink = max(ink, stroke * lerp(0.55, 1.0, weight));
    }
    return ink;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void hatch(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    float3 original = clamp(rikka_effect_straight(color), 0.0, 1.0);
    float scale = rikka_effect_scale(push.size);
    float spacing = max(parameters.strokes.spacing * scale, 2.0);
    uint seed = parameters.seed + uint(floor(max(push.time, 0.0) * parameters.rate)) * 7919u;
    float2 position = float2(pixel) + 0.5;
    float tone = rikka_luma(original);
    float ink = hatching(parameters.strokes, position, spacing, tone, seed);
    ink = max(ink, edge_ink(parameters.edges, pixel, max(int(round(scale)), 1)));
    float grain = rikka_effect_noise(float3(position * 0.7 / max(scale, 0.5), 3.0), seed + 101u) * 0.5 + 0.5;
    ink *= lerp(0.7, 1.0, grain) * parameters.darkness;
    float3 paper = rikka_effect_encode(parameters.paper);
    float3 base = lerp(paper, paper * original / max(rikka_luma(original), 0.25), clamp(parameters.color, 0.0, 1.0));
    float3 shown = lerp(base, rikka_effect_encode(parameters.pencil), clamp(ink, 0.0, 1.0));
    rikka_effect_store(push.target, pixel, float4(shown * color.a, color.a));
}
