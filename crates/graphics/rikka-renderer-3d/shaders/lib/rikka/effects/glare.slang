__exported import rikka.effects.pyramid;

static const int RIKKA_GLARE_TAPS = 24;
static const float RIKKA_GLARE_DEEPEST = 3.0;

float4 rikka_glare_extract(uint source, int2 pixel, uint2 size, float threshold, float knee) {
    float2 from = float2(rikka_effect_image_size(source));
    float2 ratio = from / float2(size);
    float2 corner = float2(pixel) * ratio;
    float3 sum = float3(0.0);
    float total = 0.0;
    int2 steps = max(int2(round(ratio * 0.5)), int2(1));
    for (int y = 0; y < steps.y; ++y) {
        for (int x = 0; x < steps.x; ++x) {
            float2 at = corner + (float2(x, y) + 0.5) * ratio / float2(steps);
            float3 color = max(rikka_effect_straight(rikka_effect_sample(source, at / from)), float3(0.0));
            float luma = rikka_luma(color);
            float weight = 1.0 / (1.0 + luma);
            sum += color * rikka_effect_bright(luma, threshold, knee) * weight;
            total += weight;
        }
    }
    return float4(sum / max(total, 1e-6), 1.0);
}

float3 rikka_glare_lod(uint base, uint middle, uint coarse, uint coarsest, float2 uv, float lod) {
    float level = clamp(lod, 0.0, RIKKA_GLARE_DEEPEST);
    uint low = uint(floor(level));
    float t = level - float(low);
    uint images[4] = { base, middle, coarse, coarsest };
    float3 below = rikka_effect_sample(images[low], uv).rgb;
    if (t <= 0.0 || low >= 3u) {
        return below;
    }
    return lerp(below, rikka_effect_sample(images[low + 1u], uv).rgb, t);
}

float3 rikka_glare_spectrum(float position) {
    return 0.5 + 0.5 * cos(RIKKA_TAU * (position * 0.75 + float3(0.0, 1.0 / 3.0, 2.0 / 3.0)));
}

struct RikkaGlareRays {
    int count;
    float angle;
    float reach;
    float falloff;
    float shift;
};

float3 rikka_glare_rays(uint base, uint middle, uint coarse, uint coarsest, float2 position, float2 size, RikkaGlareRays rays) {
    float reach = max(rays.reach, 1e-3);
    float falloff = max(rays.falloff, 0.05);
    float spacing = reach / float(RIKKA_GLARE_TAPS);
    float lod = log2(max(spacing, 1.0));
    float norm = (falloff + 1.0) / reach;
    float3 sum = float3(0.0);
    int count = max(rays.count, 1);
    for (int ray = 0; ray < count; ++ray) {
        float angle = rays.angle + float(ray) * RIKKA_TAU / float(count);
        float2 direction = float2(cos(angle), -sin(angle));
        for (int tap = 0; tap < RIKKA_GLARE_TAPS; ++tap) {
            float u = (float(tap) + 0.5) / float(RIKKA_GLARE_TAPS);
            float weight = pow(1.0 - u, falloff) * spacing * norm;
            float3 tint = lerp(float3(1.0), 2.0 * rikka_glare_spectrum(u), clamp(rays.shift, 0.0, 1.0));
            float2 at = (position - direction * u * reach) / size;
            sum += rikka_glare_lod(base, middle, coarse, coarsest, at, lod) * weight * tint;
        }
    }
    return sum / float(count);
}
