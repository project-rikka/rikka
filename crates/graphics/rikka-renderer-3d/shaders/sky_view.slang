import rikka.sky.atmosphere;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int STEPS = 32;
static const float AIRGLOW_SHELL = 0.972;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 size = int2(rikka_image_size(rikka_storage_rgba16f[push.target]));
    int2 texel = int2(global_id.xy);
    if (any(texel >= size)) {
        return;
    }
    RikkaSky sky = push.frame.look.sky;
    float3 direction = rikka_sky_view_direction((float2(texel) + 0.5) / float2(size));
    float3 origin = rikka_viewer_origin();
    float span = rikka_atmosphere_path(origin, direction);
    float3 light = float3(0.0);
    float3 throughput = float3(1.0);
    float previous = 0.0;
    for (int i = 0; i < STEPS; ++i) {
        float w = (float(i) + 1.0) / float(STEPS);
        float next = span * w * w;
        float t = 0.5 * (previous + next);
        float3 at = origin + direction * t;
        RikkaMedium medium = rikka_medium(length(at) - RIKKA_PLANET_RADIUS, sky.haze);
        float3 source = rikka_atmosphere_light(sky, at, medium, direction);
        rikka_scatter_step(source, medium.extinction, next - previous, light, throughput);
        previous = next;
    }
    if (!rikka_hits_ground(origin, direction)) {
        float slant = 1.0 / sqrt(max(1.0 - AIRGLOW_SHELL * (1.0 - direction.y * direction.y),
                                     0.06));
        light += RIKKA_AIRGLOW * slant * throughput;
    }
    rikka_storage_rgba16f[push.target][uint2(texel)] =
        float4(light, dot(throughput, float3(1.0 / 3.0)));
}
