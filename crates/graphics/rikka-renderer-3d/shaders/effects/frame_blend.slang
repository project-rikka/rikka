import rikka.effect;

struct Parameters {
    [Default(0.5)] [Range(0.0, 1.0)] [Label("shutter (fraction of 360°)")] float shutter;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint last;
    uint target;
    uint keep;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void blend(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    float4 color = rikka_effect_load(push.source, pixel);
    rikka_effect_store(push.keep, pixel, color);
    float weight = 0.5 * clamp(push.parameters.shutter, 0.0, 1.0);
    if (weight <= 0.0) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float4 before = rikka_effect_sample(push.last, rikka_effect_uv(pixel, push.size));
    rikka_effect_store(push.target, pixel, lerp(color, before, weight));
}
