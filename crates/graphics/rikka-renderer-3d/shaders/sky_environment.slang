import rikka.sky.dome;
import rikka.sky;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 size = int2(rikka_image_size(rikka_storage_rgba16f[push.target]));
    int2 texel = int2(global_id.xy);
    if (any(texel >= size)) {
        return;
    }
    RikkaFrameRef frame = push.frame;
    RikkaSky sky = frame.look.sky;
    float3 direction = rikka_octahedral_direction((float2(texel) + 0.5) / float2(size));
    float3 ground = rikka_ground_radiance(sky, rikka_sky_irradiance(frame, float3(0.0, 1.0, 0.0)));
    RikkaDome dome = rikka_sky_dome(sky, direction, frame.view.time,
                                    rikka_clear_sky_ambient(sky), ground);
    rikka_storage_rgba16f[push.target][uint2(texel)] = float4(dome.radiance, 1.0);
}
