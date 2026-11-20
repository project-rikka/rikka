__exported import rikka.effects.pyramid;

struct Parameters {
    [Default(8.0)] [Range(0.0, 400.0)] [Label("radius (σ, px at 1080p)")] float radius;
};

float rikka_gaussian_sigma(Parameters* parameters, uint2 full) {
    return max(parameters.radius, 0.0) * rikka_effect_scale(full);
}

uint rikka_gaussian_deepest(float sigma, bool varies) {
    if (!varies) {
        return rikka_pyramid_plan(sigma).level;
    }
    float variance = sigma * sigma;
    for (uint level = 1u; level <= RIKKA_PYRAMID_LEVELS; ++level) {
        if (rikka_pyramid_level_variance(level) >= variance) {
            return level;
        }
    }
    return RIKKA_PYRAMID_LEVELS;
}
