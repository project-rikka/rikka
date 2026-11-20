import rikka.effect;

struct Parameters {
    [Default(160.0)] [Range(8.0, 1080.0)] [Label("rows")] float cells;
    [Options("square|round|diamond")] [Default(0)] uint shape;
    [Default(0.0)] [Range(0.0, 0.8)] float gap;
    [Default(0.0, 0.0, 0.0)] [Color] float3 background;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint ROUND = 1u;
static const uint DIAMOND = 2u;

[shader("compute")]
[numthreads(8, 8, 1)]
void cells(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float cell = max(float(push.size.y) / max(parameters.cells, 1.0), 1.0);
    float2 position = float2(pixel) + 0.5;
    float2 index = floor(position / cell);
    float2 centre = (index + 0.5) * cell;
    int taps = clamp(int(cell * 0.5), 2, 6);
    float4 color = float4(0.0);
    for (int y = 0; y < taps; ++y) {
        for (int x = 0; x < taps; ++x) {
            float2 at = index * cell + (float2(x, y) + 0.5) / float(taps) * cell;
            color += rikka_effect_sample(push.source, at / float2(push.size));
        }
    }
    color /= float(taps * taps);
    float2 local = (position - centre) / cell * 2.0;
    float edge = parameters.shape == ROUND ? length(local)
        : parameters.shape == DIAMOND ? abs(local.x) + abs(local.y)
        : max(abs(local.x), abs(local.y));
    float limit = 1.0 - parameters.gap;
    float smoothing = 2.0 / cell;
    float inside = parameters.gap <= 0.0 && parameters.shape == 0u
        ? 1.0
        : 1.0 - smoothstep(limit - smoothing, limit + smoothing, edge);
    float4 background = float4(rikka_effect_encode(parameters.background), 1.0) * color.a;
    rikka_effect_store(push.target, pixel, lerp(background, color, inside));
}
