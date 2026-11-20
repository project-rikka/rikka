import rikka.effects.channel_mixer;

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void mix(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    float4 color = rikka_effect_load(push.source, pixel);
    float3 mixed = rikka_mix_channels(*push.parameters, rikka_effect_straight(color));
    rikka_effect_store(push.target, pixel, float4(mixed * color.a, color.a));
}
