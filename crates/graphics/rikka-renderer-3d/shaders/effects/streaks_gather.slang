import rikka.effects.streaks;

struct Push {
    Parameters* parameters;
    uint image;
    uint base;
    uint middle;
    uint coarse;
    uint coarsest;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void streak(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size) || push.parameters.amount <= 0.0) {
        return;
    }
    Parameters parameters = *push.parameters;
    uint2 full = rikka_effect_image_size(push.image);
    float ratio = float(push.size.y) / float(full.y);
    RikkaGlareRays rays;
    rays.count = parameters.count;
    rays.angle = parameters.angle;
    rays.reach = parameters.length * rikka_effect_scale(full) * ratio;
    rays.falloff = parameters.falloff;
    rays.shift = parameters.shift;
    float2 position = float2(id.xy) + 0.5;
    float3 streaks = rikka_glare_rays(push.base, push.middle, push.coarse, push.coarsest, position, float2(push.size), rays);
    rikka_effect_store(push.target, int2(id.xy), float4(streaks, 1.0));
}
