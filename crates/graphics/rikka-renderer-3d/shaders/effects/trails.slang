import rikka.effect;

struct Parameters {
    [Default(0.85)] [Range(0.0, 0.99)] [Label("persistence (kept per frame)")] float persistence;
    [Options("lighten|blend")] [Default(0)] uint mode;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint history;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint BLEND = 1u;

[shader("compute")]
[numthreads(8, 8, 1)]
void echo(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    float persistence = clamp(parameters.persistence, 0.0, 0.99);
    if (persistence <= 0.0) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float4 echoed = rikka_effect_sample(push.history, rikka_effect_uv(pixel, push.size)) * persistence;
    float4 result = parameters.mode == BLEND
        ? color * (1.0 - persistence) + echoed
        : max(color, echoed);
    rikka_effect_store(push.target, pixel, result);
}
