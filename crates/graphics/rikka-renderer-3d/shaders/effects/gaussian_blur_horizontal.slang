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
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void horizontal(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size) || push.size_map != RIKKA_NONE) {
        return;
    }
    uint2 full = rikka_effect_image_size(push.image);
    float sigma = rikka_gaussian_sigma(push.parameters, full);
    if (sigma <= 0.0) {
        return;
    }
    RikkaPyramidPlan plan = rikka_pyramid_plan(sigma);
    uint2 region = rikka_pyramid_region(full, plan.level);
    if (any(id.xy >= region)) {
        return;
    }
    uint stack[RIKKA_PYRAMID_LEVELS] = {
        push.level1, push.level2, push.level3, push.level4,
        push.level5, push.level6, push.level7, push.level8
    };
    uint source = plan.level == 0u ? push.image : stack[plan.level - 1u];
    int2 pixel = int2(id.xy);
    rikka_effect_store(push.target, pixel,
        rikka_pyramid_gaussian(source, pixel, float2(1.0, 0.0), plan.residual, float2(region)));
}
