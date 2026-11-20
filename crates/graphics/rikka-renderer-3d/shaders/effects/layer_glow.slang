import rikka.effects.layer_glow;

struct Push {
    Parameters* parameters;
    uint source;
    uint halo;
    uint layers;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float2 DIRECTIONS[8] = {
    float2(1.0, 0.0), float2(-1.0, 0.0), float2(0.0, 1.0), float2(0.0, -1.0),
    float2(0.7071, 0.7071), float2(-0.7071, 0.7071), float2(0.7071, -0.7071), float2(-0.7071, -0.7071)
};

[shader("compute")]
[numthreads(8, 8, 1)]
void combine(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    bool inside = rikka_glow_chosen(parameters, push.layers, pixel, push.size);
    float width = parameters.outline * rikka_effect_scale(push.size);
    float rim = 0.0;
    if (!inside && width > 0.0) {
        for (int ring = 1; ring <= 2; ++ring) {
            for (int direction = 0; direction < 8; ++direction) {
                float reach = max(width * float(ring) * 0.5, 1.0);
                int2 offset = int2(round(DIRECTIONS[direction] * reach));
                if (rikka_glow_chosen(parameters, push.layers, pixel + offset, push.size)) {
                    rim = max(rim, clamp(width + 1.0 - length(float2(offset)), 0.0, 1.0));
                }
            }
        }
    }
    float halo = rikka_effect_sample(push.halo, rikka_effect_uv(pixel, push.size)).r;
    float3 glow_color = rikka_effect_encode(parameters.color);
    float glow = inside ? 0.0 : clamp(halo * parameters.intensity, 0.0, 1.0);
    float3 straight = rikka_effect_straight(color);
    float3 shown = straight * color.a;
    float alpha = color.a;
    shown = shown + glow_color * glow * (1.0 - shown);
    alpha = alpha + glow * (1.0 - alpha);
    float solid = max(rim, inside ? clamp(parameters.fill, 0.0, 1.0) : 0.0);
    shown = lerp(shown, glow_color, solid);
    alpha = lerp(alpha, 1.0, solid);
    rikka_effect_store(push.target, pixel, float4(shown, alpha));
}
