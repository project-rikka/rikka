import rikka.effects.streaks;

struct Push {
    Parameters* parameters;
    uint source;
    uint streaks;
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
    float2 region = float2(rikka_effect_image_size(push.streaks));
    float2 position = (float2(pixel) + 0.5) * region / float2(push.size);
    float3 streaks = rikka_pyramid_bspline(push.streaks, position, region).rgb;
    color.rgb += streaks * parameters.amount * parameters.tint;
    rikka_effect_store(push.target, pixel, color);
}
