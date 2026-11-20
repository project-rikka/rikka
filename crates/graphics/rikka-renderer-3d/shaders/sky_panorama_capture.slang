import rikka.sky.panorama;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint source;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float3 captured(RikkaSky sky, Texture2D image, int2 texel) {
    float3 color = rikka_fetch(image, texel, 0).rgb;
    color = select(isfinite(color), max(color, float3(0.0)), float3(0.0));
    float bright = rikka_luma(color);
    if (bright > sky.panorama_clamp) {
        color *= sky.panorama_clamp / bright;
    }
    return color;
}

float3 sampled(RikkaSky sky, Texture2D image, int2 size, float2 uv) {
    float2 at = uv * float2(size) - 0.5;
    float2 corner = floor(at);
    float2 blend = at - corner;
    int2 low = int2(corner);
    int x0 = (low.x % size.x + size.x) % size.x;
    int x1 = (x0 + 1) % size.x;
    int y0 = clamp(low.y, 0, size.y - 1);
    int y1 = clamp(low.y + 1, 0, size.y - 1);
    float3 a = captured(sky, image, int2(x0, y0));
    float3 b = captured(sky, image, int2(x1, y0));
    float3 c = captured(sky, image, int2(x0, y1));
    float3 d = captured(sky, image, int2(x1, y1));
    return lerp(lerp(a, b, blend.x), lerp(c, d, blend.x), blend.y);
}

float3 averaged(RikkaSky sky, Texture2D image, int2 size, uint2 texel, float2 scale) {
    float2 start = float2(texel) * scale;
    float2 end = start + scale;
    int2 first = int2(floor(start));
    int2 last = min(int2(ceil(end)), size) - 1;
    float3 sum = float3(0.0);
    float weight = 0.0;
    for (int y = first.y; y <= last.y; ++y) {
        float rows = min(end.y, float(y + 1)) - max(start.y, float(y));
        float polar = (float(y) + 0.5) / float(size.y) * RIKKA_PI;
        float row_weight = rows * sin(polar);
        for (int x = first.x; x <= last.x; ++x) {
            float columns = min(end.x, float(x + 1)) - max(start.x, float(x));
            float texel_weight = row_weight * columns;
            sum += captured(sky, image, int2(x, y)) * texel_weight;
            weight += texel_weight;
        }
    }
    return sum / max(weight, 1e-12);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = global_id.xy;
    if (any(texel >= uint2(RIKKA_PANORAMA_WIDTH, RIKKA_PANORAMA_HEIGHT))) {
        return;
    }
    RikkaSky sky = push.frame.look.sky;
    Texture2D image = rikka_images[NonUniformResourceIndex(push.source)];
    int2 size = int2(rikka_image_size(image, 0));
    float2 scale = float2(size) / float2(RIKKA_PANORAMA_WIDTH, RIKKA_PANORAMA_HEIGHT);
    float3 color = all(scale >= 1.0)
        ? averaged(sky, image, size, texel, scale)
        : sampled(sky, image, size,
                  (float2(texel) + 0.5) / float2(RIKKA_PANORAMA_WIDTH, RIKKA_PANORAMA_HEIGHT));
    color = min(color * sky.panorama_scale, float3(RIKKA_HALF_LARGEST));
    rikka_storage_rgba16f[NonUniformResourceIndex(push.target)][texel] = float4(color, 1.0);
}
