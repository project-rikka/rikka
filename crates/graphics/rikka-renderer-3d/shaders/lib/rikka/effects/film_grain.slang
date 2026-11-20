__exported import rikka.effect;

struct Parameters {
    [Default(0.06)] [Range(0.0, 1.0)] float amount;
    [Default(0.6)] [Range(0.0, 1.0)] [Label("midtone response")] float response;
    [Default(1.6)] [Range(0.5, 8.0)] [Label("grain size (px at 1080p)")] float size;
    [Default(0.0)] [Range(0.0, 1.0)] [Label("colour grain")] float color;
    [Default(24.0)] [Range(0.0, 120.0)] [Hint("advanced")] float rate;
};

static const float RIKKA_GRAIN_SPREAD = 1.7;

float rikka_grain_lattice(int2 cell, uint seed, uint channel) {
    uint key = rikka_hash(uint(cell.x) + rikka_hash(uint(cell.y) + rikka_hash(seed * 3u + channel)));
    return float(key >> 8) / 16777216.0 * 2.0 - 1.0;
}

float rikka_grain_noise(float2 position, uint seed, uint channel) {
    int2 cell = int2(floor(position));
    float2 f = position - float2(cell);
    float2 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    float a = rikka_grain_lattice(cell, seed, channel);
    float b = rikka_grain_lattice(cell + int2(1, 0), seed, channel);
    float c = rikka_grain_lattice(cell + int2(0, 1), seed, channel);
    float d = rikka_grain_lattice(cell + int2(1, 1), seed, channel);
    return lerp(lerp(a, b, u.x), lerp(c, d, u.x), u.y) * RIKKA_GRAIN_SPREAD;
}

float rikka_grain_response(float luma, float response) {
    float midtones = 4.0 * luma * (1.0 - luma);
    return lerp(1.0, midtones, clamp(response, 0.0, 1.0));
}
