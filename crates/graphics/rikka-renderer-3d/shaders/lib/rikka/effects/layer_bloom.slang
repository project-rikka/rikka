__exported import rikka.effects.bloom;

struct Parameters {
    [Default(1)] uint mask;
    [Default(0.5)] [Range(-6.0, 10.0)] [Label("threshold (EV over middle grey)")] float threshold;
    [Default(1.0)] [Range(0.0, 4.0)] [Label("knee (EV)")] float knee;
    [Default(0.7)] [Range(0.0, 8.0)] float intensity;
    [Default(24.0)] [Range(2.0, 96.0)] [Label("radius (px at 1080p)")] float radius;
    [Default(0.6)] [Range(0.0, 2.0)] [Label("wide halo")] float wide;
    [Default(1.0, 1.0, 1.0)] [Color] float3 tint;
    [Default(1.3)] [Range(0.0, 2.0)] float saturation;
};

float4 rikka_layer_bloom_extract(Parameters parameters, uint source, uint layers, int2 pixel, uint2 size) {
    uint2 full = rikka_effect_layers_size(layers);
    float2 texel = 1.0 / float2(full);
    float2 uv = rikka_effect_uv(pixel, size);
    float3 sum = float3(0.0);
    for (int y = 0; y < 2; ++y) {
        for (int x = 0; x < 2; ++x) {
            float2 at = uv + (float2(x, y) * 2.0 - 1.0) * texel;
            int2 probe = int2(at * float2(full));
            if ((rikka_effect_load_layers_clamped(layers, probe, full) & parameters.mask) != 0u) {
                float3 color = rikka_effect_sample(source, at).rgb;
                sum += rikka_bloom_bright(color, parameters.threshold, parameters.knee);
            }
        }
    }
    return rikka_bloom_saturated(sum * 0.25, parameters.saturation);
}

extension Parameters : IBloomParameters {
    static float sigma(Parameters* parameters, uint2 size) {
        return rikka_bloom_sigma(parameters.radius, size);
    }

    float4 combined(float4 color, float3 near, float3 halo) {
        return rikka_bloom_combine(color, near, halo, wide, intensity, tint);
    }
}
