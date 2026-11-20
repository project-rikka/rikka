import rikka.common;

struct Push {
    uint source;
    uint destination;
    uint2 size;
    float sharpness;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float LOBE_LIMIT = 0.25 - 1.0 / 16.0;

float max3(float3 v) {
    return max(v.x, max(v.y, v.z));
}

float3 perceptual(float3 color) {
    float3 rgb = max(color, 0.0);
    return rgb / (1.0 + max3(rgb));
}

float3 linear_of(float3 color) {
    return color / max(1.0 - max3(color), 1e-4);
}

float4 tap(int2 pixel) {
    int2 clamped = clamp(pixel, int2(0), int2(push.size) - 1);
    return rikka_fetch(rikka_images[NonUniformResourceIndex(push.source)], clamped, 0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (any(uint2(pixel) >= push.size)) {
        return;
    }
    float4 centre = tap(pixel);
    float3 e = perceptual(centre.rgb);
    float3 b = perceptual(tap(pixel + int2(0, -1)).rgb);
    float3 d = perceptual(tap(pixel + int2(-1, 0)).rgb);
    float3 f = perceptual(tap(pixel + int2(1, 0)).rgb);
    float3 h = perceptual(tap(pixel + int2(0, 1)).rgb);

    float3 low = min(min(b, d), min(f, h));
    float3 high = max(max(b, d), max(f, h));
    float3 hit_low = low / (4.0 * high + 1e-5);
    float3 hit_high = (1.0 - high) / (4.0 * low - 4.0 - 1e-5);
    float3 lobes = max(-hit_low, hit_high);
    float lobe = max(-LOBE_LIMIT, min(max3(lobes), 0.0)) * push.sharpness;

    float3 sharpened = (lobe * (b + d + f + h) + e) / (4.0 * lobe + 1.0);
    rikka_storage_rgba16f[NonUniformResourceIndex(push.destination)][pixel] =
        float4(linear_of(clamp(sharpened, 0.0, 0.999)), centre.a);
}
