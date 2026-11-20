import rikka.push;

static const uint RIKKA_BATCH = 64u;

struct Box {
    float3 low;
    float3 high;
};

groupshared float4 candidates[RIKKA_BATCH];

float3 view_space(RikkaViewRef view, float2 ndc, float depth) {
    float4 world = mul(view.inverse_view_projection, float4(ndc, depth, 1.0));
    return mul(view.view, float4(world.xyz / world.w, 1.0)).xyz;
}

float slice_depth(RikkaViewRef view, uint slice) {
    return exp2((float(slice) - view.cluster_depth_bias) / view.cluster_depth_scale);
}

Box bounds(RikkaViewRef view, uint2 first_tile, uint2 end_tile, uint first_slice, uint end_slice) {
    float2 low_pixel = float2(first_tile * view.cluster_tile);
    float2 high_pixel = min(float2(end_tile * view.cluster_tile), view.viewport);
    float near_z = -slice_depth(view, first_slice);
    float far_z = -slice_depth(view, end_slice);

    Box box;
    box.low = float3(1e30);
    box.high = float3(-1e30);
    [unroll]
    for (int i = 0; i < 4; ++i) {
        float2 pixel = float2((i & 1) != 0 ? high_pixel.x : low_pixel.x,
                              (i & 2) != 0 ? high_pixel.y : low_pixel.y);
        float2 ndc = pixel * view.inverse_viewport * 2.0 - 1.0;
        float3 a = view_space(view, ndc, 1.0);
        float3 b = view_space(view, ndc, 0.5);
        float3 along = b - a;
        [unroll]
        for (int j = 0; j < 2; ++j) {
            float z = j == 0 ? near_z : far_z;
            float t = abs(along.z) > 1e-12 ? (z - a.z) / along.z : 0.0;
            float3 corner = a + along * t;
            box.low = min(box.low, corner);
            box.high = max(box.high, corner);
        }
    }
    return box;
}

bool sphere_hits(Box box, float3 center, float radius) {
    float3 offset = center - clamp(center, box.low, box.high);
    return dot(offset, offset) <= radius * radius;
}

[shader("compute")]
[numthreads(4, 4, 4)]
void compute_main(
    uint3 global_id: SV_DispatchThreadID,
    uint3 group_id: SV_GroupID,
    uint local_index: SV_GroupIndex)
{
    const uint3 group_size = uint3(4u, 4u, 4u);
    RikkaFrameRef frame = rikka_push.frame;
    RikkaViewRef view = frame.view;
    uint3 cluster = global_id;
    bool owned = all(cluster < view.cluster_counts);

    uint3 group_first = group_id * group_size;
    uint3 group_end = min(group_first + group_size, view.cluster_counts);
    Box group = bounds(view, group_first.xy, group_end.xy, group_first.z, group_end.z);
    Box own = bounds(view, cluster.xy, cluster.xy + 1u, cluster.z, cluster.z + 1u);

    uint stride = view.cluster_stride;
    uint index = (cluster.z * view.cluster_counts.y + cluster.y) * view.cluster_counts.x
        + cluster.x;
    uint offset = index * stride;
    uint written = 0u;

    for (uint first = view.directional_count; first < view.light_count; first += RIKKA_BATCH) {
        uint light_index = first + local_index;
        float4 candidate = float4(0.0, 0.0, 0.0, -1.0);
        if (light_index < view.light_count) {
            RikkaLight light = frame.lights[light_index];
            float3 center = mul(view.view, float4(light.position, 1.0)).xyz;
            if (sphere_hits(group, center, light.range)) {
                candidate = float4(center, light.range);
            }
        }
        candidates[local_index] = candidate;
        GroupMemoryBarrierWithGroupSync();

        uint batch = min(RIKKA_BATCH, view.light_count - first);
        for (uint j = 0u; owned && j < batch && written < stride; ++j) {
            float4 light = candidates[j];
            if (light.w >= 0.0 && sphere_hits(own, light.xyz, light.w)) {
                frame.cluster_indices[offset + written] = first + j;
                written += 1u;
            }
        }
        GroupMemoryBarrierWithGroupSync();
    }
    if (owned) {
        frame.clusters[index].count = written;
    }
}
