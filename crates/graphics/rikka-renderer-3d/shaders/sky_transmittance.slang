import rikka.sky.atmosphere;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int STEPS = 40;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 size = int2(rikka_image_size(rikka_storage_rgba16f[push.target]));
    int2 texel = int2(global_id.xy);
    if (any(texel >= size)) {
        return;
    }
    RikkaSky sky = push.frame.look.sky;
    float r;
    float mu;
    rikka_transmittance_params((float2(texel) + 0.5) / float2(size), float2(size), r, mu);
    float3 origin = float3(0.0, r, 0.0);
    float3 direction = float3(sqrt(max(1.0 - mu * mu, 0.0)), mu, 0.0);
    float span = max(rikka_ray_sphere(origin, direction, RIKKA_ATMOSPHERE_RADIUS), 0.0);
    float step = span / float(STEPS);
    float3 depth = float3(0.0);
    for (int i = 0; i < STEPS; ++i) {
        float3 at = origin + direction * ((float(i) + 0.5) * step);
        depth += rikka_medium(length(at) - RIKKA_PLANET_RADIUS, sky.haze).extinction * step;
    }
    rikka_storage_rgba16f[push.target][uint2(texel)] = float4(exp(-depth), 1.0);
}
