import rikka.effects.lens_dirt;

struct Push {
    Parameters* parameters;
    uint source;
    uint glare;
    uint pattern;
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
    float2 uv = rikka_effect_uv(pixel, push.size);
    float4 color = rikka_effect_load(push.source, pixel);
    float3 glare = rikka_effect_sample(push.glare, uv).rgb;
    float3 dirt = rikka_effect_sample(push.pattern, uv).rgb;
    float3 added = glare * dirt * parameters.intensity * parameters.tint;
    rikka_effect_store(push.target, pixel, float4(color.rgb + added, max(color.a, clamp(rikka_luma(added), 0.0, 1.0))));
}
