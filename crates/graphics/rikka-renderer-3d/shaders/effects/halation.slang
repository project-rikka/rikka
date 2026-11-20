import rikka.effects.halation;

struct Push {
    Parameters* parameters;
    uint source;
    uint level1;
    uint level2;
    uint level3;
    uint level4;
    uint level5;
    uint level6;
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
    uint stack[RIKKA_HALATION_LEVELS] = {
        push.level1, push.level2, push.level3, push.level4, push.level5, push.level6
    };
    float sigma = parameters.radius * rikka_effect_scale(push.size);
    float variance = sigma * sigma;
    uint lower = 1u;
    for (uint level = 2u; level <= RIKKA_HALATION_LEVELS; ++level) {
        if (rikka_pyramid_level_variance(level) > variance) {
            break;
        }
        lower = level;
    }
    float spread = rikka_pyramid_level(stack[lower - 1u], pixel, push.size, lower).r;
    if (lower < RIKKA_HALATION_LEVELS) {
        uint upper = lower + 1u;
        float from = rikka_pyramid_level_variance(lower);
        float to = rikka_pyramid_level_variance(upper);
        float t = clamp((variance - from) / (to - from), 0.0, 1.0);
        spread = lerp(spread, rikka_pyramid_level(stack[upper - 1u], pixel, push.size, upper).r, t);
    }
    color.rgb += parameters.tint * spread * parameters.amount;
    rikka_effect_store(push.target, pixel, color);
}
