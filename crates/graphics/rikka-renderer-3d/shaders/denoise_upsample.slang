import rikka.denoise;

struct Push {
    RikkaFrameRef frame;
    uint source;
    uint guide;
    uint guide_half;
    uint target;
    uint2 size;
    uint2 half_size;
    float4 empty;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float DEPTH_TOLERANCE = 0.03;
static const float NORMAL_POWER = 16.0;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 pixel = int2(global_id.xy);
    if (any(uint2(pixel) >= push.size)) {
        return;
    }
    RikkaGuide here = rikka_guide_load(push.guide, pixel);
    if (!here.covered) {
        rikka_storage_rgba16f[push.target][uint2(pixel)] = push.empty;
        return;
    }
    float2 centre = (float2(pixel) + 0.5) * 0.5 - 0.5;
    int2 base = int2(floor(centre));
    int2 highest = int2(push.half_size) - 1;
    float4 sum = float4(0.0);
    float total = 0.0;
    float4 nearest = push.empty;
    float nearest_weight = -1.0;
    [unroll]
    for (int y = -1; y <= 2; ++y) {
        [unroll]
        for (int x = -1; x <= 2; ++x) {
            int2 tap = clamp(base + int2(x, y), int2(0), highest);
            RikkaGuide there = rikka_guide_load(push.guide_half, tap);
            float similar = rikka_guide_weight(here, there.normal, there.covered ? there.depth : 0.0,
                DEPTH_TOLERANCE, NORMAL_POWER);
            float2 offset = float2(base + int2(x, y)) - centre;
            float spatial = exp(-dot(offset, offset));
            float4 value = rikka_fetch(rikka_images[push.source], tap, 0);
            if (similar > nearest_weight) {
                nearest_weight = similar;
                nearest = value;
            }
            float w = similar * spatial;
            sum += value * w;
            total += w;
        }
    }
    rikka_storage_rgba16f[push.target][uint2(pixel)] = total > 1e-4 ? sum / total : nearest;
}
