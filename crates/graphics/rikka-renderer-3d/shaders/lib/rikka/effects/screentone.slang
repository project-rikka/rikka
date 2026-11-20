__exported import rikka.effects.screen;

struct Levels {
    [Default(0.14)] [Range(0.0, 1.0)] float black;
    [Default(0.36)] [Range(0.0, 1.0)] float dark;
    [Default(0.74)] [Range(0.0, 1.0)] float light;
    [Default(0.55)] [Range(0.0, 1.0)] float dark_tone;
    [Default(0.2)] [Range(0.0, 1.0)] float light_tone;
    [Default(0.03)] [Range(0.0, 0.25)] float softness;
};

struct Parameters {
    [Options("dot|line")] [Default(0)] uint pattern;
    [Default(7.0)] [Range(2.0, 48.0)] [Label("cell (px at 1080p)")] float cell;
    [Default(0.785398)] [Range(-3.14159, 3.14159)] [Hint("angle")] float angle;
    [Group("levels")] Levels levels;
    [Default(0.004, 0.004, 0.006)] [Color] [Group("colours")] float3 ink;
    [Default(0.92, 0.90, 0.85)] [Color] [Group("colours")] float3 paper;
    [Default(0.0)] [Range(0.0, 1.0)] [Group("colours")] float color;
    [Default(0.75)] [Range(0.1, 4.0)] [Label("edge softness (px)")] float softness;
};

float4 rikka_screentone(Parameters parameters, uint source, int2 pixel, uint2 size) {
    float4 color = rikka_effect_load(source, pixel);
    float3 original = clamp(rikka_effect_straight(color), 0.0, 1.0);
    float tone = rikka_luma(original);
    Levels levels = parameters.levels;
    float soft = max(levels.softness, 1e-3);
    float black = 1.0 - smoothstep(levels.black - soft, levels.black + soft, tone);
    float coverage = lerp(levels.dark_tone, levels.light_tone, smoothstep(levels.dark - soft, levels.dark + soft, tone))
        * (1.0 - smoothstep(levels.light - soft, levels.light + soft, tone));
    float cell = max(parameters.cell * rikka_effect_scale(size), 2.0);
    float2 position = float2(pixel) + 0.5 - 0.5 * float2(size);
    RikkaScreenCell screen = rikka_screen_cell(position, cell, parameters.angle);
    float dots = rikka_screen_ink(screen.local, coverage, parameters.pattern, parameters.softness / cell);
    float ink = max(black, dots);
    float3 paper = lerp(rikka_effect_encode(parameters.paper), original, clamp(parameters.color, 0.0, 1.0));
    float3 shown = lerp(paper, rikka_effect_encode(parameters.ink), ink);
    return float4(shown * color.a, color.a);
}
