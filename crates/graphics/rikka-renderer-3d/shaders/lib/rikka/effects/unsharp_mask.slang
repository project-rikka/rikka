__exported import rikka.effects.blur;

struct Parameters {
    [Default(0.6)] [Range(0.0, 4.0)] float amount;
    [Default(2.0)] [Range(0.3, 32.0)] [Label("radius (σ, px at 1080p)")] float radius;
    [Default(0.0)] [Range(0.0, 1.0)] [Label("threshold (contrast)")] float threshold;
};

extension Parameters : IBlurParameters {
    static float sigma(Parameters* parameters, uint2 size) {
        return parameters.radius * rikka_effect_scale(size);
    }
}
