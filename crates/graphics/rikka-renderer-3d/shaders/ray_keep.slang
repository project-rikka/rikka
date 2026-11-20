import rikka.denoise;

struct Push {
    RikkaFrameRef frame;
    uint lit;
    uint full;
    uint half;
    uint full_history;
    uint half_history;
    uint radiance;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float BRIGHTEST = 64.0;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 pixel = global_id.xy;
    if (any(pixel >= push.size)) {
        return;
    }
    rikka_storage_rgba16f[push.full_history][pixel] = rikka_fetch(rikka_images[push.full], int2(pixel), 0);
    if (any((pixel & 1u) != 0u)) {
        return;
    }
    uint2 half_pixel = pixel >> 1u;
    rikka_storage_rgba16f[push.half_history][half_pixel] = rikka_fetch(rikka_images[push.half], int2(half_pixel), 0);
    if (push.radiance != RIKKA_NONE) {
        int2 highest = int2(push.size) - 1;
        float3 sum = float3(0.0);
        [unroll]
        for (int y = 0; y < 2; ++y) {
            [unroll]
            for (int x = 0; x < 2; ++x) {
                sum += rikka_fetch(rikka_images[push.lit], min(int2(pixel) + int2(x, y), highest), 0).rgb;
            }
        }
        float3 radiance = min(sum * 0.25, float3(BRIGHTEST));
        if (any(isnan(radiance))) {
            radiance = float3(0.0);
        }
        rikka_storage_rgba16f[push.radiance][half_pixel] = float4(radiance, 1.0);
    }
}
