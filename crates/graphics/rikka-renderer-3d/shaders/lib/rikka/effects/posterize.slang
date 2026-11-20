__exported import rikka.effect;

struct Dither {
    [Options("none|bayer|noise")] [Default(0)] uint pattern;
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
    [Default(1.0)] [Range(1.0, 8.0)] float scale;
};

struct Parameters {
    [Default(5)] [Range(2, 64)] int levels;
    [Options("rgb|luma")] [Default(0)] uint mode;
    [Group("dither")] Dither dither;
};

static const uint RIKKA_DITHER_BAYER = 1u;
static const uint RIKKA_DITHER_NOISE = 2u;
static const uint RIKKA_POSTERIZE_LUMA = 1u;

float rikka_bayer(uint2 cell) {
    uint x = cell.x & 7u;
    uint y = (cell.y ^ cell.x) & 7u;
    uint index = ((y & 1u) << 5) | ((x & 1u) << 4) | ((y & 2u) << 2) | ((x & 2u) << 1)
        | ((y & 4u) >> 1) | ((x & 4u) >> 2);
    return (float(index) + 0.5) / 64.0;
}

float rikka_dither_threshold(Dither dither, int2 pixel, float scale) {
    uint2 cell = uint2(floor(float2(pixel) / max(dither.scale * scale, 1.0)));
    if (dither.pattern == RIKKA_DITHER_BAYER) {
        return (rikka_bayer(cell) - 0.5) * clamp(dither.amount, 0.0, 1.0);
    }
    if (dither.pattern == RIKKA_DITHER_NOISE) {
        return (rikka_interleaved_gradient(float2(cell)) - 0.5) * clamp(dither.amount, 0.0, 1.0);
    }
    return 0.0;
}

float3 rikka_posterize(Parameters parameters, float3 color, float offset) {
    float steps = float(max(parameters.levels, 2) - 1);
    if (parameters.mode == RIKKA_POSTERIZE_LUMA) {
        float luma = rikka_luma(color);
        float banded = clamp(floor(luma * steps + 0.5 + offset) / steps, 0.0, 1.0);
        return luma > 1e-5 ? color * (banded / luma) : float3(banded);
    }
    return clamp(floor(color * steps + 0.5 + offset) / steps, 0.0, 1.0);
}
