import rikka.effects.posterize;

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void quantize(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    float offset = rikka_dither_threshold(parameters.dither, pixel, rikka_effect_scale(push.size));
    float3 banded = rikka_posterize(parameters, rikka_effect_straight(color), offset);
    rikka_effect_store(push.target, pixel, float4(banded * color.a, color.a));
}
