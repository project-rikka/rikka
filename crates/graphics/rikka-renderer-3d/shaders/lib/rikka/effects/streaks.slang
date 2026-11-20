__exported import rikka.effects.glare;

struct Parameters {
    [Default(0.5)] [Range(0.0, 4.0)] float amount;
    [Default(4)] [Range(2, 16)] int count;
    [Default(0.0)] [Range(-3.14159, 3.14159)] [Hint("angle")] float angle;
    [Default(240.0)] [Range(8.0, 1080.0)] [Label("length (px at 1080p)")] float length;
    [Default(2.0)] [Range(0.25, 8.0)] float falloff;
    [Default(0.0)] [Range(0.0, 1.0)] [Label("colour shift")] float shift;
    [Default(2.0)] [Range(-4.0, 10.0)] [Label("threshold (EV over middle grey)")] float threshold;
    [Default(1.0)] [Range(0.0, 4.0)] [Label("knee (EV)")] float knee;
    [Default(1.0, 1.0, 1.0)] [Color] float3 tint;
};
