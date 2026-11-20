import rikka.effect;

struct Mask {
    [Options("full|bottom|ellipse")] [Default(0)] uint region;
    [Default(0.5)] [Range(0.0, 1.0)] float extent;
    [Default(0.25)] [Range(0.0, 1.0)] float feather;
    bool by_layers;
    [Default(1)] uint layers;
    [Default(0.0)] [Range(0.0, 1000.0)] float near;
    [Default(100000.0)] [Range(0.0, 100000.0)] float far;
    [Texture("data")] uint texture;
};

struct Parameters {
    [Default(9.0)] [Range(0.0, 40.0)] [Label("strength (px at 1080p)")] float strength;
    [Default(55.0)] [Range(4.0, 400.0)] [Label("scale (px at 1080p)")] float scale;
    [Default(1.2)] [Range(0.0, 8.0)] float speed;
    [Default(2.5)] [Range(1.0, 6.0)] float stretch;
    [Default(3)] uint seed;
    [Group("mask")] Mask mask;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint depth;
    uint layers;
    uint target;
    uint2 size;
    RikkaFrameRef frame;
    float time;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint BOTTOM = 1u;
static const uint ELLIPSE = 2u;
static const float FARTHEST = 100000.0;

float region_weight(Mask mask, float2 uv, float aspect) {
    float soft = max(mask.feather, 1e-3);
    if (mask.region == BOTTOM) {
        return smoothstep(1.0 - mask.extent - soft, 1.0 - mask.extent + soft, uv.y);
    }
    if (mask.region == ELLIPSE) {
        float distance = length((uv - 0.5) * float2(aspect, 1.0)) / max(mask.extent, 1e-3);
        return 1.0 - smoothstep(1.0 - soft, 1.0 + soft, distance);
    }
    return 1.0;
}

float mask_at(Mask mask, int2 pixel, float2 uv, float aspect) {
    float weight = region_weight(mask, uv, aspect);
    if (mask.by_layers) {
        weight *= (rikka_effect_load_layers(push.layers, pixel) & mask.layers) != 0u ? 1.0 : 0.0;
    }
    float depth = min(rikka_effect_linear_depth(push.frame, rikka_effect_load(push.depth, pixel).r), FARTHEST);
    float fade = max(0.02 * (mask.far - mask.near), 1e-3);
    weight *= smoothstep(mask.near - fade, mask.near, depth) * (1.0 - smoothstep(mask.far, mask.far + fade, depth));
    if (mask.texture != RIKKA_NONE) {
        weight *= rikka_effect_sample_repeat(mask.texture, uv).r;
    }
    return weight;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void shimmer(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float2 uv = rikka_effect_uv(pixel, push.size);
    float scale = rikka_effect_scale(push.size);
    float aspect = rikka_effect_aspect(push.size);
    float weight = mask_at(parameters.mask, pixel, uv, aspect);
    float4 color = rikka_effect_load(push.source, pixel);
    if (weight <= 0.0 || parameters.strength <= 0.0) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float2 position = float2(pixel) / max(parameters.scale * scale, 1.0);
    float rise = push.time * parameters.speed;
    float3 flow = float3(position.x, position.y / parameters.stretch + rise, push.time * 0.35);
    float2 wobble = float2(
        rikka_effect_fbm(flow, parameters.seed, 3),
        rikka_effect_fbm(flow + float3(5.2, 1.3, 2.7), parameters.seed + 11u, 3));
    float2 displaced = uv + wobble * parameters.strength * scale * weight / float2(push.size);
    rikka_effect_store(push.target, pixel, rikka_effect_sample(push.source, displaced));
}
