import rikka.sky.panorama;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint source;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = uint2(global_id.x, global_id.y + RIKKA_PANORAMA_HEIGHT);
    RWTexture2D<float4> atlas = rikka_storage_rgba16f[NonUniformResourceIndex(push.target)];
    for (uint level = 1u; level < RIKKA_PANORAMA_LEVELS; ++level) {
        uint4 rect = rikka_panorama_level(level);
        if (any(texel < rect.xy) || any(texel >= rect.xy + rect.zw)) {
            continue;
        }
        uint span = 1u << level;
        uint2 first = (texel - rect.xy) * span;
        float3 sum = float3(0.0);
        float weight = 0.0;
        for (uint y = first.y; y < first.y + span; ++y) {
            float polar = (float(y) + 0.5) / float(RIKKA_PANORAMA_HEIGHT) * RIKKA_PI;
            float row_weight = sin(polar);
            float3 row = float3(0.0);
            for (uint x = first.x; x < first.x + span; ++x) {
                row += atlas[uint2(x, y)].rgb;
            }
            sum += row * row_weight;
            weight += row_weight * float(span);
        }
        atlas[texel] = float4(sum / max(weight, 1e-12), 1.0);
        return;
    }
}
