import rikka.effects.gaussian_blur;

struct Push {
    Parameters* parameters;
    uint image;
    uint size_map;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void vertical(uint3 id: SV_DispatchThreadID) {
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
    int2 pixel = int2(id.xy);
    rikka_effect_store(push.target, pixel,
        rikka_pyramid_gaussian(push.source, pixel, float2(0.0, 1.0), plan.residual, float2(region)));
}
