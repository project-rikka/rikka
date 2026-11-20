__exported import rikka.effect;

struct Parameters {
    [Default(8.0)] [Range(0.0, 64.0)] [Label("spread (px at 1080p)")] float amount;
    [Default(9)] [Range(3, 24)] int samples;
    [Default(0.5, 0.5)] [Range(0.0, 1.0)] float2 center;
    [Default(1.6)] [Range(0.5, 4.0)] float falloff;
};

float3 rikka_spectrum(float t) {
    float3 peaks = float3(0.85, 0.5, 0.15);
    return max(float3(1.0) - abs(float3(t) - peaks) / float3(0.38, 0.32, 0.38), float3(0.0));
}

float4 rikka_disperse(Parameters parameters, uint source, int2 pixel, uint2 size) {
    float2 uv = rikka_effect_uv(pixel, size);
    float aspect = rikka_effect_aspect(size);
    float2 offset = (uv - parameters.center) * float2(aspect, 1.0);
    float corner = length(float2(aspect, 1.0) * 0.5);
    float reach = pow(clamp(length(offset) / corner, 0.0, 1.5), parameters.falloff);
    float2 direction = length(offset) > 1e-6 ? normalize(offset) : float2(0.0);
    float2 shift = direction * reach * parameters.amount * rikka_effect_scale(size) / float2(size);
    int count = clamp(parameters.samples, 3, 24);
    float4 sum = float4(0.0);
    float3 total = float3(0.0);
    for (int index = 0; index < count; ++index) {
        float t = (float(index) + 0.5) / float(count);
        float3 weight = rikka_spectrum(t);
        float4 tap = rikka_effect_sample(source, uv + shift * (t - 0.5) * 2.0);
        sum += float4(tap.rgb * weight, tap.a * dot(weight, float3(1.0 / 3.0)));
        total += weight;
    }
    return float4(sum.rgb / max(total, float3(1e-4)), sum.a / max(dot(total, float3(1.0 / 3.0)), 1e-4));
}
