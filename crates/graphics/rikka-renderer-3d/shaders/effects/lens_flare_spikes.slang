import rikka.effects.lens_flare;

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

static const float SPIKE_FALLOFF = 2.5;

[shader("compute")]
[numthreads(8, 8, 1)]
void spikes(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size) || push.parameters.amount <= 0.0) {
        return;
    }
    Parameters parameters = *push.parameters;
    if (parameters.starburst.amount <= 0.0) {
        rikka_effect_store(push.target, int2(id.xy), float4(0.0, 0.0, 0.0, 1.0));
        return;
    }
    uint2 full = rikka_effect_image_size(push.image);
    float ratio = float(push.size.y) / float(full.y);
    int blades = max(parameters.starburst.blades, 3);
    RikkaGlareRays rays;
    rays.count = (blades & 1) == 0 ? blades : 2 * blades;
    rays.angle = parameters.starburst.rotation;
    rays.reach = parameters.starburst.length * rikka_effect_scale(full) * ratio;
    rays.falloff = SPIKE_FALLOFF;
    rays.shift = parameters.ghosts.chromatic * 0.5;
    float2 position = float2(id.xy) + 0.5;
    float3 spikes = rikka_glare_rays(push.base, push.middle, push.coarse, push.coarsest, position, float2(push.size), rays);
    rikka_effect_store(push.target, int2(id.xy), float4(spikes, 1.0));
}
