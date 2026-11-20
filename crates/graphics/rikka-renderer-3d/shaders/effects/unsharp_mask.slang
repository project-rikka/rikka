import rikka.effects.unsharp_mask;

struct Push {
    Parameters* parameters;
    uint source;
    uint blurred;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void sharpen(uint3 id: SV_DispatchThreadID) {
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
    float4 detail = color - rikka_effect_load(push.blurred, pixel);
    float weight = 1.0;
    if (parameters.threshold > 0.0) {
        float contrast = abs(rikka_luma(detail.rgb));
        weight = smoothstep(parameters.threshold, 2.0 * parameters.threshold, contrast);
    }
    float4 sharpened = color + detail * parameters.amount * weight;
    rikka_effect_store(push.target, pixel, float4(max(sharpened.rgb, float3(0.0)), color.a));
}
