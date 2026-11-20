import rikka.ray;
import rikka.denoise;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint samples;
    uint2 size;
    float radius;
    float strength;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float HEMISPHERE_MEAN = 2.0 / 3.0;

float occluded(RikkaFrameRef frame, float3 origin, float3 direction, float reach) {
    RikkaRayHit hit = rikka_ray_closest(frame, origin, direction, reach);
    if (!hit.hit) {
        return 0.0;
    }
    float near = hit.distance / reach;
    return 1.0 - near * near;
}

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
        rikka_storage_rgba16f[push.target][texel] = float4(1.0, 0.0, 0.0, 0.0);
        return;
    }
    float3 position = rikka_world_of(frame, pixel);
    float3 origin = rikka_ray_lift(position, guide.normal, guide.depth);
    float open = 0.0;
    float3 bent = float3(0.0);
    uint samples = max(push.samples, 1u);
    for (uint s = 0u; s < samples; ++s) {
        float2 noise = rikka_ray_noise(uint2(pixel), trace.frame * samples + s, 4u);
        float3 direction = rikka_ray_cosine(guide.normal, noise);
        float seen = 1.0 - occluded(frame, origin, direction, push.radius);
        open += seen;
        bent += direction * seen;
    }
    float raw = open / float(samples);
    float visibility = pow(max(raw, 0.0), push.strength);
    float3 mean = bent / (HEMISPHERE_MEAN * float(samples));
    float3 bend = mean * (visibility / max(raw, 1e-4)) - visibility * guide.normal;
    rikka_storage_rgba16f[push.target][texel] = float4(visibility, bend);
}
