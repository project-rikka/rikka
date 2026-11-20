__exported import rikka.effects.screen;

struct Parameters {
    [Options("dot|line")] [Default(0)] uint pattern;
    [Options("mono|cmyk")] [Default(0)] uint inks;
    [Default(10.0)] [Range(2.0, 64.0)] [Label("cell (px at 1080p)")] float cell;
    [Default(0.785398)] [Range(-3.14159, 3.14159)] [Hint("angle")] float angle;
    [Default(1.1)] [Range(0.25, 3.0)] float contrast;
    [Default(0.75)] [Range(0.1, 4.0)] [Label("edge softness (px)")] float softness;
    [Default(0.008, 0.007, 0.010)] [Color] [Group("colours")] float3 ink;
    [Default(0.86, 0.80, 0.66)] [Color] [Group("colours")] float3 paper;
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
};

static const uint RIKKA_HALFTONE_CMYK = 1u;
static const float RIKKA_HALFTONE_CYAN = -0.523599;
static const float RIKKA_HALFTONE_MAGENTA = 0.523599;
static const float RIKKA_HALFTONE_YELLOW = -0.785398;

float rikka_halftone_coverage(Parameters parameters, float ink) {
    return clamp((ink - 0.5) * parameters.contrast + 0.5, 0.0, 1.0);
}

float4 rikka_cmyk(float3 rgb) {
    float3 c = clamp(rgb, 0.0, 1.0);
    float black = 1.0 - max(c.r, max(c.g, c.b));
    float3 inks = black < 0.9999 ? (1.0 - c - black) / (1.0 - black) : float3(0.0);
    return float4(inks, black);
}

float rikka_halftone_channel(Parameters parameters, uint source, float2 position, float cell, float angle, int channel, float smoothing, uint2 size) {
    RikkaScreenCell screen = rikka_screen_cell(position, cell, parameters.angle + angle);
    float4 average = rikka_screen_average(source, screen.centre, cell, size);
    float4 inks = rikka_cmyk(rikka_effect_straight(average));
    float coverage = rikka_halftone_coverage(parameters, inks[channel]);
    return rikka_screen_ink(screen.local, coverage, parameters.pattern, smoothing);
}

float4 rikka_halftone(Parameters parameters, uint source, int2 pixel, uint2 size) {
    float4 color = rikka_effect_load(source, pixel);
    float3 original = rikka_effect_straight(color);
    float cell = max(parameters.cell * rikka_effect_scale(size), 2.0);
    float smoothing = parameters.softness / cell;
    float2 position = float2(pixel) + 0.5 - 0.5 * float2(size);
    float3 paper = rikka_effect_encode(parameters.paper);
    float3 printed;
    if (parameters.inks == RIKKA_HALFTONE_CMYK) {
        float cyan = rikka_halftone_channel(parameters, source, position, cell, RIKKA_HALFTONE_CYAN, 0, smoothing, size);
        float magenta = rikka_halftone_channel(parameters, source, position, cell, RIKKA_HALFTONE_MAGENTA, 1, smoothing, size);
        float yellow = rikka_halftone_channel(parameters, source, position, cell, RIKKA_HALFTONE_YELLOW, 2, smoothing, size);
        float black = rikka_halftone_channel(parameters, source, position, cell, 0.0, 3, smoothing, size);
        printed = paper * (1.0 - float3(cyan, magenta, yellow)) * (1.0 - black);
    } else {
        RikkaScreenCell screen = rikka_screen_cell(position, cell, parameters.angle);
        float4 average = rikka_screen_average(source, screen.centre, cell, size);
        float tone = rikka_luma(clamp(rikka_effect_straight(average), 0.0, 1.0));
        float ink = rikka_screen_ink(screen.local, rikka_halftone_coverage(parameters, 1.0 - tone), parameters.pattern, smoothing);
        printed = lerp(paper, rikka_effect_encode(parameters.ink), ink);
    }
    float3 shown = lerp(original, printed, clamp(parameters.amount, 0.0, 1.0));
    return float4(shown * color.a, color.a);
}
