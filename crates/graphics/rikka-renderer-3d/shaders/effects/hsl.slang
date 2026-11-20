import rikka.effects.hsl;

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void adjust(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    float4 color = rikka_effect_load(push.source, pixel);
    float3 linear = rikka_decode_srgb(rikka_effect_straight(color));
    float3 adjusted = rikka_encode_srgb(rikka_hsl_adjust(*push.parameters, linear));
    rikka_effect_store(push.target, pixel, float4(adjusted * color.a, color.a));
}
