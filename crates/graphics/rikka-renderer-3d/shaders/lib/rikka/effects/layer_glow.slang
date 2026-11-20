__exported import rikka.effects.blur;

struct Parameters {
    [Default(1)] uint mask;
    [Default(1.0, 0.42, 0.06)] [Color] float3 color;
    [Default(20.0)] [Range(1.0, 64.0)] [Label("radius (px at 1080p)")] float radius;
    [Default(2.5)] [Range(0.0, 8.0)] float intensity;
    [Default(2.5)] [Range(0.0, 8.0)] [Label("outline (px at 1080p)")] float outline;
    [Default(0.0)] [Range(0.0, 1.0)] float fill;
};

bool rikka_glow_chosen(Parameters parameters, uint layers, int2 pixel, uint2 size) {
    return (rikka_effect_load_layers_clamped(layers, pixel, size) & parameters.mask) != 0u;
}

extension Parameters : IBlurParameters {
    static float sigma(Parameters* parameters, uint2 size) {
        Parameters values = *parameters;
        return 0.5 * values.radius * rikka_effect_scale(size);
    }

    override static void store(uint image, int2 pixel, float4 value) {
        rikka_effect_store_r32(image, pixel, value.r);
    }
}
