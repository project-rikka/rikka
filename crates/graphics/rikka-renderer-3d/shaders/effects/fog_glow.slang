import rikka.effects.fog_glow;

struct Push {
    Parameters* parameters;
    uint source;
    uint folded;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void combine(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    int2 pixel = int2(id.xy);
    float4 color = rikka_effect_load(push.source, pixel);
    if (parameters.amount <= 0.0) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float3 glow = rikka_pyramid_level(push.folded, pixel, push.size, 1u).rgb;
    color.rgb += glow * parameters.amount * parameters.tint;
    rikka_effect_store(push.target, pixel, color);
}
