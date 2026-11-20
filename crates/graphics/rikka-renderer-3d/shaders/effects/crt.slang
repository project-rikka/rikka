import rikka.effect;

struct Parameters {
    [Default(0.12)] [Range(0.0, 0.5)] float curvature;
    [Default(0.55)] [Range(0.0, 1.0)] float scanlines;
    [Default(270.0)] [Range(60.0, 1080.0)] [Label("lines")] float lines;
    [Options("none|aperture|shadow|slot")] [Default(1)] uint mask;
    [Default(0.3)] [Range(0.0, 1.0)] [Label("mask strength")] float mask_strength;
    [Default(3.0)] [Range(1.0, 12.0)] [Label("mask pitch (px at 1080p)")] float pitch;
    [Default(0.25)] [Range(0.0, 1.0)] float glow;
    [Default(0.6)] [Range(0.0, 4.0)] [Label("misconvergence (px at 1080p)")] float misconvergence;
    [Default(0.35)] [Range(0.0, 1.0)] float vignette;
    [Default(1.15)] [Range(0.5, 2.0)] float brightness;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint APERTURE = 1u;
static const uint SHADOW = 2u;
static const uint SLOT = 3u;

float3 linear_at(float2 uv) {
    return rikka_decode_srgb(clamp(rikka_effect_straight(rikka_effect_sample(push.source, uv)), 0.0, 1.0));
}

float3 phosphor_mask(uint mask, float2 position, float pitch) {
    float2 cell = position / pitch;
    if (mask == APERTURE) {
        int column = int(floor(cell.x)) % 3;
        return float3(column == 0 ? 1.0 : 0.0, column == 1 ? 1.0 : 0.0, column == 2 ? 1.0 : 0.0);
    }
    if (mask == SHADOW) {
        float shifted = cell.x + (int(floor(cell.y)) % 2 == 0 ? 0.0 : 1.5);
        int column = int(floor(shifted)) % 3;
        return float3(column == 0 ? 1.0 : 0.0, column == 1 ? 1.0 : 0.0, column == 2 ? 1.0 : 0.0);
    }
    if (mask == SLOT) {
        int column = int(floor(cell.x)) % 3;
        int group = int(floor(cell.x / 3.0));
        float row = frac(cell.y / 3.0 + (group % 2 == 0 ? 0.0 : 0.5));
        float gate = row > 0.85 ? 0.0 : 1.0;
        return float3(column == 0 ? 1.0 : 0.0, column == 1 ? 1.0 : 0.0, column == 2 ? 1.0 : 0.0) * gate;
    }
    return float3(1.0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void scan(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float scale = rikka_effect_scale(push.size);
    float2 uv = rikka_effect_uv(pixel, push.size);
    float2 centred = uv * 2.0 - 1.0;
    float2 bent = centred * (1.0 + parameters.curvature * float2(centred.y * centred.y, centred.x * centred.x));
    float2 screen = bent * 0.5 + 0.5;
    float alpha = rikka_effect_load(push.source, pixel).a;
    if (any(screen < 0.0) || any(screen > 1.0)) {
        rikka_effect_store(push.target, pixel, float4(0.0, 0.0, 0.0, alpha));
        return;
    }
    float lines = max(parameters.lines, 1.0);
    float row = screen.y * lines;
    float row_centre = (floor(row) + 0.5) / lines;
    float within = frac(row) - 0.5;
    float shift = parameters.misconvergence * scale / float(push.size.x);
    float2 beam_uv = float2(screen.x, lerp(screen.y, row_centre, parameters.scanlines));
    float3 color = float3(
        linear_at(beam_uv + float2(shift, 0.0)).r,
        linear_at(beam_uv).g,
        linear_at(beam_uv - float2(shift, 0.0)).b);
    float brightness = rikka_luma(color);
    float sigma = lerp(0.18, 0.42, sqrt(clamp(brightness, 0.0, 1.0)));
    float beam = exp(-0.5 * within * within / (sigma * sigma));
    color *= lerp(1.0, beam * 1.6, parameters.scanlines);
    float2 texel = 1.5 * scale / float2(push.size);
    float3 halo = 0.25 * (linear_at(screen + float2(texel.x, 0.0)) + linear_at(screen - float2(texel.x, 0.0))
        + linear_at(screen + float2(0.0, texel.y)) + linear_at(screen - float2(0.0, texel.y)));
    color += halo * parameters.glow * 0.5;
    float3 mask = phosphor_mask(parameters.mask, float2(pixel) + 0.5, max(parameters.pitch * scale, 1.0));
    color *= lerp(float3(1.0), mask * 3.0, parameters.mask_strength / 3.0 * 2.0);
    float edge = 1.0 - parameters.vignette * pow(dot(centred, centred) * 0.5, 1.5);
    float2 rim = smoothstep(0.0, 0.01, screen) * smoothstep(0.0, 0.01, 1.0 - screen);
    color *= edge * rim.x * rim.y * parameters.brightness;
    rikka_effect_store(push.target, pixel, float4(rikka_encode_srgb(clamp(color, 0.0, 1.0)) * alpha, alpha));
}
