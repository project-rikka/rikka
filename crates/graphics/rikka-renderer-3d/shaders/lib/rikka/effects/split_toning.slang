__exported import rikka.effect;

struct Parameters {
    [Default(0.013, 0.147, 0.342)] [Color] float3 shadows;
    [Default(1.0, 0.42, 0.12)] [Color] float3 highlights;
    [Default(0.0)] [Range(-1.0, 1.0)] float balance;
    [Default(0.6)] [Range(0.0, 1.0)] float amount;
};

float3 rikka_soft_light(float3 base, float3 blend) {
    float3 low = 2.0 * base * blend + base * base * (1.0 - 2.0 * blend);
    float3 high = sqrt(max(base, float3(0.0))) * (2.0 * blend - 1.0) + 2.0 * base * (1.0 - blend);
    return select(blend < float3(0.5), low, high);
}

float3 rikka_split_tone(Parameters parameters, float3 encoded) {
    float3 base = clamp(encoded, 0.0, 1.0);
    float t = clamp(rikka_luma(base) + 0.5 * parameters.balance, 0.0, 1.0);
    float3 shadows = rikka_encode_srgb(parameters.shadows);
    float3 highlights = rikka_encode_srgb(parameters.highlights);
    float3 shadow = 0.5 + shadows - rikka_luma(shadows);
    float3 highlight = 0.5 + highlights - rikka_luma(highlights);
    float3 toned = rikka_soft_light(base, clamp(lerp(float3(0.5), shadow, 1.0 - t), 0.0, 1.0));
    toned = rikka_soft_light(toned, clamp(lerp(float3(0.5), highlight, t), 0.0, 1.0));
    return lerp(encoded, toned, clamp(parameters.amount, 0.0, 1.0));
}
