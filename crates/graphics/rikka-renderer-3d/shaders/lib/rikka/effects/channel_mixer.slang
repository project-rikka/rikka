__exported import rikka.effect;

struct Parameters {
    [Default(1.0, 0.0, 0.0)] [Range(-2.0, 2.0)] [Group("mix")] [Label("red from")] float3 red;
    [Default(0.0, 1.0, 0.0)] [Range(-2.0, 2.0)] [Group("mix")] [Label("green from")] float3 green;
    [Default(0.0, 0.0, 1.0)] [Range(-2.0, 2.0)] [Group("mix")] [Label("blue from")] float3 blue;
    [Group("monochrome")] bool monochrome;
    [Default(0.2126, 0.7152, 0.0722)] [Range(-2.0, 2.0)] [Group("monochrome")] [Label("grey from")] float3 grey;
    [Default(1.0)] bool preserve_luminance;
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
};

float3 rikka_mix_channels(Parameters parameters, float3 color) {
    float3 mixed = parameters.monochrome
        ? float3(dot(parameters.grey, color))
        : float3(dot(parameters.red, color), dot(parameters.green, color), dot(parameters.blue, color));
    mixed = max(mixed, float3(0.0));
    if (parameters.preserve_luminance) {
        float before = rikka_luma(color);
        float after = rikka_luma(mixed);
        mixed *= after > 1e-6 ? before / after : 0.0;
    }
    return lerp(color, mixed, clamp(parameters.amount, 0.0, 1.0));
}
