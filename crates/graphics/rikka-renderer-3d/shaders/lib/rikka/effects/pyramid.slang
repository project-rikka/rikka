__exported import rikka.effect;

static const uint RIKKA_PYRAMID_LEVELS = 8u;
static const float RIKKA_PYRAMID_REDUCE_VARIANCE = 0.75;
static const float RIKKA_PYRAMID_BSPLINE_VARIANCE = 1.0 / 3.0;
static const float RIKKA_PYRAMID_SMALLEST_RESIDUAL = 1.0;
static const float RIKKA_PYRAMID_LARGEST_RESIDUAL = 21.0;

struct RikkaPyramidPlan {
    uint level;
    float residual;
};

float rikka_pyramid_scale(uint level) {
    return exp2(float(level));
}

float rikka_pyramid_reduced_variance(uint level) {
    float area = exp2(2.0 * float(level));
    return RIKKA_PYRAMID_REDUCE_VARIANCE * (area - 1.0) / 3.0;
}

float rikka_pyramid_level_variance(uint level) {
    if (level == 0u) {
        return 0.0;
    }
    return rikka_pyramid_reduced_variance(level)
        + exp2(2.0 * float(level)) * RIKKA_PYRAMID_BSPLINE_VARIANCE;
}

uint2 rikka_pyramid_region(uint2 full, uint level) {
    uint divisor = 1u << level;
    return max((full + divisor - 1u) / divisor, uint2(1u));
}

uint rikka_pyramid_level_of(uint2 full, uint2 size) {
    return uint(round(log2(max(float(full.y), 1.0) / max(float(size.y), 1.0))));
}

RikkaPyramidPlan rikka_pyramid_plan(float sigma) {
    RikkaPyramidPlan plan;
    plan.level = 0u;
    float variance = sigma * sigma;
    for (uint level = 1u; level <= RIKKA_PYRAMID_LEVELS; ++level) {
        float area = exp2(2.0 * float(level));
        float needed = rikka_pyramid_reduced_variance(level)
            + area * (RIKKA_PYRAMID_SMALLEST_RESIDUAL + RIKKA_PYRAMID_BSPLINE_VARIANCE);
        if (variance < needed) {
            break;
        }
        plan.level = level;
    }
    if (plan.level == 0u) {
        plan.residual = sigma;
        return plan;
    }
    float area = exp2(2.0 * float(plan.level));
    float left = (variance - rikka_pyramid_reduced_variance(plan.level)) / area
        - RIKKA_PYRAMID_BSPLINE_VARIANCE;
    plan.residual = min(sqrt(max(left, 0.0)), RIKKA_PYRAMID_LARGEST_RESIDUAL);
    return plan;
}

float4 rikka_pyramid_reduce(uint source, int2 pixel, uint2 size) {
    float2 from = float2(rikka_effect_image_size(source));
    float2 center = (float2(pixel) + 0.5) * from / float2(size);
    float4 sum = float4(0.0);
    for (int y = 0; y < 2; ++y) {
        for (int x = 0; x < 2; ++x) {
            float2 offset = float2(x == 0 ? -0.75 : 0.75, y == 0 ? -0.75 : 0.75);
            sum += rikka_effect_sample(source, (center + offset) / from);
        }
    }
    return sum * 0.25;
}

float4 rikka_pyramid_fetch(uint image, float2 position, float2 region, float2 extent) {
    float2 clamped = clamp(position, float2(0.5), region - 0.5);
    return rikka_effect_sample(image, clamped / extent);
}

float4 rikka_pyramid_bspline(uint image, float2 position, float2 region) {
    float2 extent = float2(rikka_effect_image_size(image));
    float2 coordinate = position - 0.5;
    float2 base = floor(coordinate);
    float2 f = coordinate - base;
    float2 f2 = f * f;
    float2 f3 = f2 * f;
    float2 w0 = (1.0 - 3.0 * f + 3.0 * f2 - f3) / 6.0;
    float2 w1 = (4.0 - 6.0 * f2 + 3.0 * f3) / 6.0;
    float2 w2 = (1.0 + 3.0 * f + 3.0 * f2 - 3.0 * f3) / 6.0;
    float2 w3 = f3 / 6.0;
    float2 g0 = w0 + w1;
    float2 g1 = w2 + w3;
    float2 low = base - 0.5 + w1 / g0;
    float2 high = base + 1.5 + w3 / g1;
    return g0.y * (g0.x * rikka_pyramid_fetch(image, float2(low.x, low.y), region, extent)
            + g1.x * rikka_pyramid_fetch(image, float2(high.x, low.y), region, extent))
        + g1.y * (g0.x * rikka_pyramid_fetch(image, float2(low.x, high.y), region, extent)
            + g1.x * rikka_pyramid_fetch(image, float2(high.x, high.y), region, extent));
}

float4 rikka_pyramid_level(uint image, int2 pixel, uint2 full, uint level) {
    float2 region = float2(rikka_pyramid_region(full, level));
    float2 position = (float2(pixel) + 0.5) * region / float2(full);
    return rikka_pyramid_bspline(image, position, region);
}

float4 rikka_pyramid_gaussian(uint image, int2 pixel, float2 axis, float sigma, float2 region) {
    float2 extent = float2(rikka_effect_image_size(image));
    float2 center = float2(pixel) + 0.5;
    float4 sum = rikka_pyramid_fetch(image, center, region, extent);
    if (sigma < 0.05) {
        return sum;
    }
    int reach = int(ceil(sigma * 3.0));
    float falloff = -0.5 / (sigma * sigma);
    float total = 1.0;
    for (int at = 1; at <= reach; at += 2) {
        float near = exp(falloff * float(at * at));
        float far = exp(falloff * float((at + 1) * (at + 1)));
        float weight = near + far;
        float offset = (float(at) * near + float(at + 1) * far) / weight;
        float2 shift = axis * offset;
        sum += (rikka_pyramid_fetch(image, center + shift, region, extent)
            + rikka_pyramid_fetch(image, center - shift, region, extent)) * weight;
        total += 2.0 * weight;
    }
    return sum / total;
}

float4 rikka_pyramid_varied(uint source, uint levels[RIKKA_PYRAMID_LEVELS], int2 pixel, uint2 full, float sigma) {
    float variance = sigma * sigma;
    if (variance <= 0.0) {
        return rikka_effect_load(source, pixel);
    }
    uint lower = 0u;
    for (uint level = 1u; level <= RIKKA_PYRAMID_LEVELS; ++level) {
        if (rikka_pyramid_level_variance(level) > variance) {
            break;
        }
        lower = level;
    }
    float4 below = lower == 0u
        ? rikka_effect_load(source, pixel)
        : rikka_pyramid_level(levels[lower - 1u], pixel, full, lower);
    if (lower == RIKKA_PYRAMID_LEVELS) {
        return below;
    }
    uint upper = lower + 1u;
    float from = rikka_pyramid_level_variance(lower);
    float to = rikka_pyramid_level_variance(upper);
    float t = clamp((variance - from) / max(to - from, 1e-6), 0.0, 1.0);
    float4 above = rikka_pyramid_level(levels[upper - 1u], pixel, full, upper);
    return lerp(below, above, t);
}
