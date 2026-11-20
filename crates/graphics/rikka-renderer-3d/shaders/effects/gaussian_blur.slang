import rikka.effects.gaussian_blur;

struct Push {
    Parameters* parameters;
    uint image;
    uint size_map;
    uint level1;
    uint level2;
    uint level3;
    uint level4;
    uint level5;
    uint level6;
    uint level7;
    uint level8;
    uint blurred;
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
    int2 pixel = int2(id.xy);
    uint2 full = push.size;
    float sigma = rikka_gaussian_sigma(push.parameters, full);
    if (sigma <= 0.0) {
        rikka_effect_store(push.target, pixel, rikka_effect_load(push.image, pixel));
        return;
    }
    if (push.size_map != RIKKA_NONE) {
        uint stack[RIKKA_PYRAMID_LEVELS] = {
            push.level1, push.level2, push.level3, push.level4,
            push.level5, push.level6, push.level7, push.level8
        };
        float scaled = sigma * max(rikka_effect_load(push.size_map, pixel).r, 0.0);
        rikka_effect_store(push.target, pixel, rikka_pyramid_varied(push.image, stack, pixel, full, scaled));
        return;
    }
    RikkaPyramidPlan plan = rikka_pyramid_plan(sigma);
    float4 blurred = plan.level == 0u
        ? rikka_effect_load(push.blurred, pixel)
        : rikka_pyramid_level(push.blurred, pixel, full, plan.level);
    rikka_effect_store(push.target, pixel, blurred);
}
