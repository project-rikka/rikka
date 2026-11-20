import rikka.effects.film_grain;

struct Push {
    Parameters* parameters;
    uint source;
    uint grain;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void combine(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    if (parameters.amount <= 0.0) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float3 grain = rikka_effect_load(push.grain, pixel).rgb;
    float3 shown = rikka_effect_straight(color);
    float weight = rikka_grain_response(rikka_luma(clamp(shown, 0.0, 1.0)), parameters.response);
    color.rgb = max(color.rgb + grain * parameters.amount * weight * color.a, float3(0.0));
    rikka_effect_store(push.target, pixel, color);
}
