__exported import rikka.effects.blur;
__exported import rikka.effects.glare;

struct Ghosts {
    [Default(4)] [Range(0, 8)] int count;
    [Default(0.35)] [Range(0.0, 1.0)] float dispersal;
    [Default(0.4)] [Range(0.0, 1.0)] float chromatic;
};

struct Halo {
    [Default(0.3)] [Range(0.0, 2.0)] float amount;
    [Default(0.45)] [Range(0.05, 0.9)] float width;
};

struct Starburst {
    [Default(0.3)] [Range(0.0, 2.0)] float amount;
    [Default(6)] [Range(3, 12)] int blades;
    [Default(0.0)] [Range(-3.14159, 3.14159)] [Hint("angle")] float rotation;
    [Default(160.0)] [Range(8.0, 1080.0)] [Label("length (px at 1080p)")] float length;
};

struct Parameters {
    [Default(0.4)] [Range(0.0, 4.0)] float amount;
    [Default(3.0)] [Range(-4.0, 12.0)] [Label("threshold (EV over middle grey)")] float threshold;
    [Default(1.0)] [Range(0.0, 4.0)] [Label("knee (EV)")] float knee;
    [Group("ghosts")] Ghosts ghosts;
    [Group("halo")] Halo halo;
    [Group("starburst")] Starburst starburst;
    [Default(1.0, 1.0, 1.0)] [Color] float3 tint;
};

static const float RIKKA_FLARE_SOFTNESS = 1.5;

extension Parameters : IBlurParameters {
    static float sigma(Parameters* parameters, uint2 size) {
        return RIKKA_FLARE_SOFTNESS;
    }
}
