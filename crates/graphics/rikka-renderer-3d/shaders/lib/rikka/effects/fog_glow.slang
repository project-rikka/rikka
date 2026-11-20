__exported import rikka.effects.glare;

struct Parameters {
    [Default(0.25)] [Range(0.0, 2.0)] float amount;
    [Default(1.0)] [Range(0.0, 1.0)] [Label("spread")] float spread;
    [Default(2.0)] [Range(1.0, 3.0)] [Label("falloff (power law)")] float falloff;
    [Default(0.0)] [Range(-6.0, 10.0)] [Label("threshold (EV over middle grey)")] float threshold;
    [Default(2.0)] [Range(0.0, 4.0)] [Label("knee (EV)")] float knee;
    [Default(1.0, 1.0, 1.0)] [Color] float3 tint;
};
