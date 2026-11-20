__exported import rikka.effects.log_curves;

struct Shaper {
    [Options("direct|srgb|log|acescct|logc3")] [Default(0)] uint mode;
    [Default(-8.0)] [Range(-16.0, 0.0)] [Hint("advanced")] float low;
    [Default(8.0)] [Range(0.0, 16.0)] [Hint("advanced")] float high;
};

struct Domain {
    [Default(0.0, 0.0, 0.0)] [Range(-64.0, 64.0)] float3 min;
    [Default(1.0, 1.0, 1.0)] [Range(-64.0, 64.0)] float3 max;
};

struct Parameters {
    [Texture("data")] uint table;
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
    [Group("shaper")] Shaper shaper;
    [Options("shaper|display")] [Default(0)] [Label("table output")] uint output;
    [Group("domain")] [Hint("advanced")] Domain domain;
};

static const uint RIKKA_SHAPER_SRGB = 1u;
static const uint RIKKA_SHAPER_LOG = 2u;
static const uint RIKKA_SHAPER_ACESCCT = 3u;
static const uint RIKKA_SHAPER_LOGC3 = 4u;
static const uint RIKKA_LOOKUP_DISPLAY = 1u;

float3 rikka_lookup_shape(Shaper shaper, float3 value) {
    if (shaper.mode == RIKKA_SHAPER_SRGB) {
        return rikka_effect_encode(value);
    }
    if (shaper.mode == RIKKA_SHAPER_LOG) {
        float3 stops = log2(max(value, float3(1e-10)) / RIKKA_EFFECT_MID_GREY);
        return (stops - shaper.low) / max(shaper.high - shaper.low, 1e-3);
    }
    if (shaper.mode == RIKKA_SHAPER_ACESCCT) {
        return rikka_to_acescct(value);
    }
    if (shaper.mode == RIKKA_SHAPER_LOGC3) {
        return rikka_to_logc(value);
    }
    return value;
}

float3 rikka_lookup_unshape(Shaper shaper, float3 value) {
    if (shaper.mode == RIKKA_SHAPER_SRGB) {
        return rikka_decode_srgb(value);
    }
    if (shaper.mode == RIKKA_SHAPER_LOG) {
        return RIKKA_EFFECT_MID_GREY * exp2(shaper.low + value * (shaper.high - shaper.low));
    }
    if (shaper.mode == RIKKA_SHAPER_ACESCCT) {
        return rikka_from_acescct(value);
    }
    if (shaper.mode == RIKKA_SHAPER_LOGC3) {
        return rikka_from_logc(value);
    }
    return value;
}

float3 rikka_lookup_strip(uint table, float3 unit, int size) {
    float3 position = clamp(unit, 0.0, 1.0) * float(size - 1);
    int3 base = min(int3(floor(position)), int3(max(size - 2, 0)));
    float3 f = position - float3(base);
    float3 looked;
    [ForceUnroll]
    for (int channel = 0; channel < 3; ++channel) {
        float low = rikka_fetch(rikka_images[NonUniformResourceIndex(table)], int2(base[channel], 0), 0)[channel];
        float high = rikka_fetch(rikka_images[NonUniformResourceIndex(table)], int2(min(base[channel] + 1, size - 1), 0), 0)[channel];
        looked[channel] = lerp(low, high, f[channel]);
    }
    return looked;
}

float3 rikka_lookup_lattice(uint table, int3 point, int size) {
    return rikka_fetch(rikka_images[NonUniformResourceIndex(table)], int2(point.r + point.b * size, point.g), 0).rgb;
}

float3 rikka_lookup_tetrahedral(uint table, float3 unit, int size) {
    float3 position = clamp(unit, 0.0, 1.0) * float(size - 1);
    int3 base = min(int3(floor(position)), int3(size - 2));
    float3 f = position - float3(base);
    float3 c000 = rikka_lookup_lattice(table, base, size);
    float3 c111 = rikka_lookup_lattice(table, base + int3(1, 1, 1), size);
    if (f.r > f.g) {
        if (f.g > f.b) {
            float3 c100 = rikka_lookup_lattice(table, base + int3(1, 0, 0), size);
            float3 c110 = rikka_lookup_lattice(table, base + int3(1, 1, 0), size);
            return (1.0 - f.r) * c000 + (f.r - f.g) * c100 + (f.g - f.b) * c110 + f.b * c111;
        }
        if (f.r > f.b) {
            float3 c100 = rikka_lookup_lattice(table, base + int3(1, 0, 0), size);
            float3 c101 = rikka_lookup_lattice(table, base + int3(1, 0, 1), size);
            return (1.0 - f.r) * c000 + (f.r - f.b) * c100 + (f.b - f.g) * c101 + f.g * c111;
        }
        float3 c001 = rikka_lookup_lattice(table, base + int3(0, 0, 1), size);
        float3 c101 = rikka_lookup_lattice(table, base + int3(1, 0, 1), size);
        return (1.0 - f.b) * c000 + (f.b - f.r) * c001 + (f.r - f.g) * c101 + f.g * c111;
    }
    if (f.b > f.g) {
        float3 c001 = rikka_lookup_lattice(table, base + int3(0, 0, 1), size);
        float3 c011 = rikka_lookup_lattice(table, base + int3(0, 1, 1), size);
        return (1.0 - f.b) * c000 + (f.b - f.g) * c001 + (f.g - f.r) * c011 + f.r * c111;
    }
    if (f.b > f.r) {
        float3 c010 = rikka_lookup_lattice(table, base + int3(0, 1, 0), size);
        float3 c011 = rikka_lookup_lattice(table, base + int3(0, 1, 1), size);
        return (1.0 - f.g) * c000 + (f.g - f.b) * c010 + (f.b - f.r) * c011 + f.r * c111;
    }
    float3 c010 = rikka_lookup_lattice(table, base + int3(0, 1, 0), size);
    float3 c110 = rikka_lookup_lattice(table, base + int3(1, 1, 0), size);
    return (1.0 - f.g) * c000 + (f.g - f.r) * c010 + (f.r - f.b) * c110 + f.b * c111;
}
