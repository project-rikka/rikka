import rikka.bounce;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint samples;
    uint2 size;
    float reach;
    uint radiance;
    uint guide_history;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = global_id.xy;
    if (any(texel >= push.size)) {
        return;
    }
    RikkaFrameRef frame = push.frame;
    RikkaTrace trace = frame.look.trace;
    int2 pixel = rikka_full_pixel(texel, 2u, uint2(frame.view.viewport));
    RikkaGuide guide = rikka_guide(frame, pixel);
    if (!guide.covered) {
        rikka_storage_rgba16f[push.target][texel] = float4(0.0);
        return;
    }
    RikkaBounce bounce;
    bounce.radiance = push.radiance;
    bounce.guide_history = push.guide_history;
    bounce.reach = push.reach;
    float3 position = rikka_world_of(frame, pixel);
    float3 origin = rikka_ray_lift(position, guide.normal, guide.depth);
    float3 sum = float3(0.0);
    uint samples = max(push.samples, 1u);
    for (uint s = 0u; s < samples; ++s) {
        uint index = trace.frame * samples + s;
        float2 noise = rikka_ray_noise(uint2(pixel), index, 5u);
        float pick = rikka_ray_noise(uint2(pixel), index, 6u).x;
        float2 onward = rikka_ray_noise(uint2(pixel), index, 10u);
        RikkaBounceSample sample = rikka_bounce_trace(frame, bounce, origin, rikka_ray_cosine(guide.normal, noise),
            pick, onward);
        sum += min(sample.radiance, float3(RIKKA_BOUNCE_BRIGHTEST));
    }
    float3 irradiance = sum / float(samples);
    if (any(isnan(irradiance))) {
        irradiance = float3(0.0);
    }
    rikka_storage_rgba16f[push.target][texel] = float4(irradiance, 1.0);
}
