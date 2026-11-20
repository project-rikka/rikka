__exported import rikka.effects.pyramid;

struct Highlights {
    [Default(1.0)] [Range(0.0, 8.0)] float boost;
    [Default(2.0)] [Range(-4.0, 10.0)] [Label("threshold (EV over middle grey)")] float threshold;
    [Default(1.0)] [Range(0.0, 4.0)] [Label("knee (EV)")] float knee;
};

struct Parameters {
    [Default(24.0)] [Range(0.0, 200.0)] [Label("radius (px at 1080p)")] float radius;
    [Default(6)] [Range(0, 12)] [Label("blades (0 = round)")] int blades;
    [Default(0.0)] [Range(-3.14159, 3.14159)] [Hint("angle")] float rotation;
    [Default(0.1)] [Range(0.0, 1.0)] [Label("blade curvature")] float curvature;
    [Default(1.0)] [Range(0.25, 4.0)] [Label("squeeze (width / height)")] float squeeze;
    [Group("highlights")] Highlights highlights;
};

static const int RIKKA_BOKEH_MAX_RINGS = 10;

float rikka_bokeh_radius(Parameters* parameters, uint size_map, float2 uv, uint2 full) {
    float radius = max(parameters.radius, 0.0) * rikka_effect_scale(full);
    if (size_map != RIKKA_NONE) {
        radius *= max(rikka_effect_sample(size_map, uv).r, 0.0);
    }
    return radius;
}

float rikka_bokeh_polygon(float angle, int blades, float rotation, float curvature) {
    if (blades < 3) {
        return 1.0;
    }
    float sector = RIKKA_TAU / float(blades);
    float local = rikka_mod(angle - rotation, sector) - 0.5 * sector;
    float edge = cos(0.5 * sector) / cos(local);
    return lerp(edge, 1.0, clamp(curvature, 0.0, 1.0));
}

float4 rikka_bokeh_boosted(Highlights highlights, float4 color) {
    float luma = rikka_luma(max(rikka_effect_straight(color), float3(0.0)));
    float lift = 1.0 + max(highlights.boost, 0.0) * rikka_effect_bright(luma, highlights.threshold, highlights.knee);
    return float4(color.rgb * lift, color.a);
}
