__exported import rikka.effect;

static const uint RIKKA_SCREEN_LINES = 1u;
static const float RIKKA_SCREEN_ROUNDING = -0.18169011;

struct RikkaScreenCell {
    float2 centre;
    float2 local;
};

RikkaScreenCell rikka_screen_cell(float2 position, float cell, float angle) {
    float2 rotated = rikka_effect_rotate(position, -angle) / cell;
    float2 index = floor(rotated) + 0.5;
    RikkaScreenCell result;
    result.local = rotated - index;
    result.centre = rikka_effect_rotate(index * cell, angle);
    return result;
}

float4 rikka_screen_average(uint source, float2 centre, float cell, uint2 size) {
    float2 quarter = float2(0.25 * cell) / float2(size);
    float2 uv = (centre + 0.5 * float2(size)) / float2(size);
    return 0.25 * (rikka_effect_sample(source, uv + float2(-quarter.x, -quarter.y))
        + rikka_effect_sample(source, uv + float2(quarter.x, -quarter.y))
        + rikka_effect_sample(source, uv + float2(-quarter.x, quarter.y))
        + rikka_effect_sample(source, uv + float2(quarter.x, quarter.y)));
}

float rikka_screen_ink(float2 local, float coverage, uint pattern, float smoothing) {
    float c = clamp(coverage, 0.0, 1.0);
    if (c <= 0.0) {
        return 0.0;
    }
    if (c >= 1.0) {
        return 1.0;
    }
    float spot;
    float threshold;
    float slope;
    if (pattern == RIKKA_SCREEN_LINES) {
        spot = cos(RIKKA_TAU * local.y);
        threshold = cos(RIKKA_PI * c);
        slope = RIKKA_TAU * abs(sin(RIKKA_TAU * local.y));
    } else {
        spot = 0.5 * (cos(RIKKA_TAU * local.x) + cos(RIKKA_TAU * local.y));
        threshold = 1.0 - 2.0 * c + RIKKA_SCREEN_ROUNDING * sin(RIKKA_TAU * c);
        slope = RIKKA_PI * length(float2(sin(RIKKA_TAU * local.x), sin(RIKKA_TAU * local.y)));
    }
    return clamp((spot - threshold) / max(slope * smoothing, 1e-4) + 0.5, 0.0, 1.0);
}
