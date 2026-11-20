import rikka.sky.fog_light;
import rikka.sky.noise;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint level;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    RikkaFrameRef frame = push.frame;
    RikkaViewRef view = frame.view;
    RikkaSky sky = frame.look.sky;
    uint2 grid = sky.fog_grid.xy;
    uint2 cell = global_id.xy;
    if (any(cell >= grid)) {
        return;
    }
    float2 uv = (float2(cell) + 0.5) / float2(grid);
    float3 ray = rikka_view_ray(view, uv * 2.0 - 1.0);
    float along = max(dot(ray, view.camera_forward), 1e-3);
    float jitter = rikka_interleaved_gradient(float2(cell));
    float3 ambient = rikka_fog_ambient(frame, sky, ray);

    float3 light = float3(0.0);
    float3 throughput = float3(1.0);
    for (int slice = 0; slice < RIKKA_FOG_SLICES; ++slice) {
        float near = rikka_fog_slice_depth(slice);
        float far = rikka_fog_slice_depth(slice + 1);
        float depth = lerp(near, far, jitter);
        float3 at = view.camera_position + ray * (depth / along);
        float density = rikka_fog_density_at(sky, at.y);
        if (density > 0.0) {
            float3 lit = ambient + rikka_fog_lights(frame, sky, at, ray, depth);
            rikka_scatter_step(density * sky.fog_color * lit, float3(density),
                               (far - near) / along, light, throughput);
        }
        rikka_storage_rgba16f[push.target][uint2(rikka_tiled_texel(grid, RIKKA_FOG_COLUMNS, cell, slice))] =
            float4(light, dot(throughput, float3(1.0 / 3.0)));
    }
}
