import rikka.denoise;

struct Push {
    RikkaFrameRef frame;
    uint source;
    uint guide;
    uint moments;
    uint target;
    uint moments_copy;
    uint2 size;
    uint scale;
    uint channels;
    int step;
    float variance_scale;
    uint tiles;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float DEPTH_TOLERANCE = 0.02;
static const float NORMAL_POWER = 64.0;
static const float LUMINANCE_SPREAD = 4.0;
static const float KERNEL[3] = { 0.375, 0.25, 0.0625 };
static const float VARIANCE_FLOOR = 1.0 / (512.0 * 512.0);

float4 load(uint image, int2 texel) {
    return rikka_fetch(rikka_images[NonUniformResourceIndex(image)], texel, 0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = global_id.xy;
    if (any(texel >= push.size)) {
        return;
    }
    float4 centre = load(push.source, int2(texel));
    float4 moments = load(push.moments, int2(texel));
    if (push.moments_copy != RIKKA_NONE) {
        rikka_storage_rgba16f[push.moments_copy][texel] = moments;
    }
    RikkaGuide here = rikka_guide_load(push.guide, int2(texel));
    float variance = max(moments.w * push.variance_scale, 0.0);
    bool quiet = variance < VARIANCE_FLOOR;
    if (!here.covered || quiet || rikka_tile_settled(push.tiles, texel, push.size)) {
        rikka_storage_rgba16f[push.target][texel] = centre;
        return;
    }
    float centre_luminance = rikka_signal_luminance(centre, push.channels);
    float spread = LUMINANCE_SPREAD * sqrt(variance) + 1e-4;
    int2 highest = int2(push.size) - 1;
    float4 sum = float4(0.0);
    float total = 0.0;
    [unroll]
    for (int y = -2; y <= 2; ++y) {
        [unroll]
        for (int x = -2; x <= 2; ++x) {
            int2 tap = int2(texel) + int2(x, y) * push.step;
            if (any(tap < int2(0)) || any(tap > highest)) {
                continue;
            }
            float4 value = load(push.source, tap);
            RikkaGuide there = rikka_guide_load(push.guide, tap);
            float w = KERNEL[abs(x)] * KERNEL[abs(y)]
                * rikka_guide_weight(here, there.normal, there.covered ? there.depth : 0.0,
                    DEPTH_TOLERANCE * float(push.step), NORMAL_POWER);
            if (x != 0 || y != 0) {
                float l = rikka_signal_luminance(value, push.channels);
                w *= exp(-abs(l - centre_luminance) / spread);
            }
            sum += value * w;
            total += w;
        }
    }
    rikka_storage_rgba16f[push.target][texel] = total > 1e-6 ? sum / total : centre;
}
