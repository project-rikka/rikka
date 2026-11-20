__exported import rikka.effects.blur;

static const float RIKKA_BLOOM_FIREFLY = 32.0;

interface IBloomParameters : IBlurParameters {
    float4 combined(float4 color, float3 near, float3 halo);
}

struct RikkaBloomPush<P : IBloomParameters> {
    P* parameters;
    uint source;
    uint near;
    uint wide;
    uint target;
    uint2 size;

    void combine(uint3 id) {
        int2 pixel = int2(id.xy);
        if (rikka_effect_outside(id.xy, size)) {
            return;
        }
        P values = *parameters;
        float2 uv = rikka_effect_uv(pixel, size);
        float4 color = rikka_effect_load(source, pixel);
        float3 near_light = rikka_effect_sample(near, uv).rgb;
        float3 wide_light = rikka_effect_sample(wide, uv).rgb;
        rikka_effect_store(target, pixel, values.combined(color, near_light, wide_light));
    }
};

float rikka_bloom_sigma(float radius, uint2 size) {
    return 0.5 * radius * rikka_effect_scale(size);
}

float3 rikka_bloom_bright(float3 color, float threshold, float knee) {
    float3 positive = max(color, float3(0.0));
    float3 bright = positive * rikka_effect_bright(rikka_luma(positive), threshold, knee);
    return bright * min(1.0, RIKKA_BLOOM_FIREFLY / max(rikka_luma(bright), 1e-6));
}

float4 rikka_bloom_saturated(float3 color, float saturation) {
    float grey = rikka_luma(color);
    return float4(max(lerp(float3(grey), color, saturation), float3(0.0)), 1.0);
}

float4 rikka_bloom_combine(float4 color, float3 near, float3 wide, float wide_amount, float intensity, float3 tint) {
    float3 added = (near + wide * wide_amount) * intensity * tint;
    return float4(color.rgb + added, max(color.a, clamp(rikka_luma(added), 0.0, 1.0)));
}
