import rikka.effects.fog_glow;

struct Push {
    Parameters* parameters;
    uint image;
    uint level1;
    uint level2;
    uint level3;
    uint level4;
    uint level5;
    uint level6;
    uint level7;
    uint level8;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void fold(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size) || push.parameters.amount <= 0.0) {
        return;
    }
    Parameters parameters = *push.parameters;
    int2 pixel = int2(id.xy);
    uint2 full = rikka_effect_image_size(push.image);
    uint stack[RIKKA_PYRAMID_LEVELS] = {
        push.level1, push.level2, push.level3, push.level4,
        push.level5, push.level6, push.level7, push.level8
    };
    uint used = clamp(uint(round(parameters.spread * float(RIKKA_PYRAMID_LEVELS))), 1u, RIKKA_PYRAMID_LEVELS);
    float exponent = 1.0 - 0.5 * clamp(parameters.falloff, 1.0, 3.0);
    float2 center = (float2(pixel) + 0.5) * 2.0;
    float3 glow = float3(0.0);
    float total = 0.0;
    for (uint level = 1u; level <= used; ++level) {
        float weight = pow(rikka_pyramid_level_variance(level), exponent);
        float2 region = float2(rikka_pyramid_region(full, level));
        float2 position = center * region / float2(full);
        float3 found = level == 1u
            ? rikka_effect_load(stack[0], pixel).rgb
            : rikka_pyramid_bspline(stack[level - 1u], position, region).rgb;
        glow += found * weight;
        total += weight;
    }
    rikka_effect_store(push.target, pixel, float4(glow / total, 1.0));
}
