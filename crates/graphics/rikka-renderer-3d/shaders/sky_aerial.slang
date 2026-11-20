import rikka.sky.fog;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 grid = rikka_image_size(rikka_storage_rgba16f[push.target])
        / uint2(RIKKA_AERIAL_COLUMNS, uint(RIKKA_AERIAL_SLICES) / RIKKA_AERIAL_COLUMNS);
    uint2 cell = global_id.xy;
    if (any(cell >= grid)) {
        return;
    }
    RikkaFrameRef frame = push.frame;
    RikkaViewRef view = frame.view;
    RikkaSky sky = frame.look.sky;
    float2 uv = (float2(cell) + 0.5) / float2(grid);
    float3 direction = rikka_view_ray(view, uv * 2.0 - 1.0);
    float3 origin = rikka_viewer_origin() + view.camera_position * RIKKA_WORLD_TO_KM;
    origin.y = max(origin.y, RIKKA_PLANET_RADIUS + 0.001);

    float3 light = float3(0.0);
    float3 throughput = float3(1.0);
    float previous = 0.0;
    for (int slice = 0; slice < RIKKA_AERIAL_SLICES; ++slice) {
        float next = rikka_aerial_distance(sky, slice);
        float t = 0.5 * (previous + next);
        float3 at = origin + direction * t;
        float altitude = max(length(at) - RIKKA_PLANET_RADIUS, 0.0);
        RikkaMedium medium = rikka_medium(altitude, sky.haze);
        float3 source = rikka_atmosphere_light(sky, at, medium, direction);
        rikka_scatter_step(source, medium.extinction, next - previous, light, throughput);
        previous = next;
        rikka_storage_rgba16f[push.target][uint2(rikka_tiled_texel(grid, RIKKA_AERIAL_COLUMNS, cell, slice))] =
            float4(light, dot(throughput, float3(1.0 / 3.0)));
    }
}
