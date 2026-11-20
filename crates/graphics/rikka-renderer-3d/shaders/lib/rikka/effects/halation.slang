__exported import rikka.effects.pyramid;

struct Parameters {
    [Default(0.6)] [Range(0.0, 4.0)] float amount;
    [Default(14.0)] [Range(1.0, 96.0)] [Label("radius (σ, px at 1080p)")] float radius;
    [Default(1.5)] [Range(-4.0, 10.0)] [Label("threshold (EV over middle grey)")] float threshold;
    [Default(1.5)] [Range(0.0, 4.0)] [Label("knee (EV)")] float knee;
    [Default(1.0, 0.22, 0.04)] [Color] float3 tint;
};

static const uint RIKKA_HALATION_LEVELS = 6u;
