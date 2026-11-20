__exported import rikka.effects.bloom;

struct Parameters {
    [Default(-3.0)] [Range(-10.0, 10.0)] [Label("threshold (EV over middle grey)")] float threshold;
    [Default(1.0)] [Range(0.0, 4.0)] [Label("knee (EV)")] float knee;
    [Default(1.5)] [Range(0.0, 8.0)] float intensity;
    [Default(28.0)] [Range(2.0, 96.0)] [Label("radius (px at 1080p)")] float radius;
    [Default(0.8)] [Range(0.0, 2.0)] [Label("wide halo")] float wide;
    [Default(1.0, 1.0, 1.0)] [Color] float3 tint;
    [Default(1.2)] [Range(0.0, 2.0)] float saturation;
};

float4 rikka_emission_bloom_extract(Parameters parameters, uint emission, int2 pixel, uint2 size) {
    float4 reduced = rikka_blur_reduce(emission, pixel, size);
    float3 bright = rikka_bloom_bright(reduced.rgb, parameters.threshold, parameters.knee);
    return rikka_bloom_saturated(bright, parameters.saturation);
}

extension Parameters : IBloomParameters {
    static float sigma(Parameters* parameters, uint2 size) {
        return rikka_bloom_sigma(parameters.radius, size);
    }

    float4 combined(float4 color, float3 near, float3 halo) {
        return rikka_bloom_combine(color, near, halo, wide, intensity, tint);
    }
}
