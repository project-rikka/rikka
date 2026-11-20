__exported import rikka.effect;

struct Parameters {
    [Default(1.5)] [Range(0.0, 16.0)] [Label("weave (px at 1080p)")] float weave;
    [Default(0.002)] [Range(0.0, 0.05)] [Hint("angle")] float rotation;
    [Default(0.06)] [Range(0.0, 1.0)] [Label("flicker (stops)")] float flicker;
    [Default(24.0)] [Range(1.0, 60.0)] [Label("frames per second")] float rate;
    [Default(0.35)] [Range(0.0, 1.0)] [Label("jitter")] float jitter;
    [Default(7)] uint seed;
};

static const float RIKKA_WEAVE_DRIFT = 0.07;

float rikka_weave_wander(float frame, uint seed, float jitter) {
    float drift = rikka_effect_fbm(float3(frame * RIKKA_WEAVE_DRIFT, 0.5, 0.5), seed, 2);
    float shake = rikka_effect_random(uint3(uint(frame), seed, 0x51u)) * 2.0 - 1.0;
    return lerp(drift, shake, clamp(jitter, 0.0, 1.0));
}

bool rikka_weave_still(Parameters parameters) {
    return parameters.weave <= 0.0 && parameters.rotation == 0.0 && parameters.flicker <= 0.0;
}
