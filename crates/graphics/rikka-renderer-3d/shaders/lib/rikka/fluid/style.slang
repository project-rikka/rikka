__exported import rikka.sky;
__exported import rikka.fluid;

static const float RIKKA_FLUID_ANIME_SHADE = 0.78;
static const float RIKKA_FLUID_ANIME_SOFTNESS = 0.1;
static const float RIKKA_FLUID_ANIME_LOCAL_FILL = 0.3;
static const float RIKKA_FLUID_GI_EQUALIZATION = 0.9;
static const float3 RIKKA_FLUID_PLAIN_SHADE = float3(0.8);
static const float RIKKA_FLUID_PLAIN_SOFTNESS = 0.1;
static const float RIKKA_FLUID_PLAIN_EDGE = 0.5;

struct RikkaFluidStyle {
    float stylized;
    float3 dark;
    float softness;
    float edge;
    float local_fill;
    float equalization;
    float3 sky_tone;
};

RikkaFluidStyle rikka_fluid_figure_style(RikkaScene scene) {
    RikkaFluidStyle style;
    style.stylized = clamp(scene.stylization, 0.0, 1.0);
    style.dark = RIKKA_FLUID_ANIME_SHADE * scene.shadow_tint;
    style.softness = max(scene.shade_softness, 0.0) + RIKKA_FLUID_ANIME_SOFTNESS;
    style.edge = clamp(scene.shade_softness * 2.0 + 0.12, 0.12, 0.5);
    style.local_fill = RIKKA_FLUID_ANIME_LOCAL_FILL;
    style.equalization = RIKKA_FLUID_GI_EQUALIZATION;
    style.sky_tone = max(scene.ambient, 0.0) * lerp(style.dark, float3(1.0), 0.5);
    return style;
}

RikkaFluidStyle rikka_fluid_plain_style() {
    RikkaFluidStyle style;
    style.stylized = 1.0;
    style.dark = RIKKA_FLUID_PLAIN_SHADE;
    style.softness = RIKKA_FLUID_PLAIN_SOFTNESS;
    style.edge = RIKKA_FLUID_PLAIN_EDGE;
    style.local_fill = 1.0;
    style.equalization = 0.0;
    style.sky_tone = float3(1.0);
    return style;
}

RikkaFluidStyle rikka_fluid_style(RikkaFrameRef frame) {
    RikkaScene scene = frame.look.scene;
    RikkaFluidStyle style = scene.styled != 0u ? rikka_fluid_figure_style(scene) : rikka_fluid_plain_style();
    style.stylized = frame.look.scene.enabled != 0u ? style.stylized : 0.0;
    return style;
}

float rikka_fluid_toward(RikkaFluidStyle style, float n_dot_l) {
    return smoothstep(-style.softness, style.softness, n_dot_l);
}

float rikka_fluid_cel_shadow(RikkaFluidStyle style, float shadow) {
    return smoothstep(0.5 - style.edge, 0.5 + style.edge, shadow);
}

float3 rikka_fluid_anime_sky(RikkaFrameRef frame, RikkaFluidStyle style, float3 sky) {
    float3 up = float3(0.0, 1.0, 0.0);
    float3 level = 0.5 * (rikka_sky_irradiance(frame, up) + rikka_sky_irradiance(frame, -up));
    return lerp(sky, level, style.equalization) * style.sky_tone;
}
