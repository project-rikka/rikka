__exported import rikka.shading;

static const float RIKKA_WET_FILM_SHEEN = 0.05;
static const float RIKKA_WET_FILM_DEEPENING = 0.5;

struct RikkaWet {
    float amount;
    float3 tint;
    float porosity;
    float film;
};

RikkaWet rikka_wet(float4 deposit, float porosity) {
    RikkaWet w;
    w.amount = clamp(deposit.a, 0.0, 1.0);
    w.tint = w.amount > 1e-4 ? clamp(deposit.rgb / w.amount, float3(0.0), float3(1.0)) : float3(1.0);
    w.porosity = clamp(porosity, 0.0, 1.0);
    float saturated = 0.65 * w.porosity;
    w.film = smoothstep(saturated, saturated + 0.35, w.amount);
    return w;
}

float rikka_wet_porosity(float roughness, float metallic) {
    return clamp((roughness - 0.35) / 0.45, 0.0, 1.0) * (1.0 - clamp(metallic, 0.0, 1.0));
}

float3 rikka_wet_albedo(float3 albedo, RikkaWet w) {
    float3 bounded = clamp(albedo, float3(0.0), float3(1.0));
    float3 soaked = albedo * lerp(float3(1.0), float3(0.25) + 0.65 * bounded, w.porosity)
        * pow(max(bounded, float3(1e-3)), float3(RIKKA_WET_FILM_DEEPENING * w.film));
    return lerp(albedo, soaked * w.tint, w.amount);
}

float rikka_wet_roughness(float roughness, RikkaWet w) {
    return roughness * lerp(1.0, 1.0 - 0.8 * w.porosity, 0.5 * w.amount);
}

float rikka_wet_film_fresnel(float cosine) {
    return RIKKA_WET_FILM_F0
        + (1.0 - RIKKA_WET_FILM_F0) * pow(1.0 - clamp(cosine, 0.0, 1.0), 5.0);
}

float rikka_wet_film_sheen(float cosine) {
    return RIKKA_WET_FILM_SHEEN + (1.0 - RIKKA_WET_FILM_SHEEN) * pow(1.0 - clamp(cosine, 0.0, 1.0), 3.0);
}

float rikka_wet_film_glint(float n_dot_h) {
    float facing = clamp(n_dot_h, 0.0, 1.0);
    return 1.4 * smoothstep(0.984, 0.993, facing) + 0.3 * pow(facing, 28.0);
}

float rikka_wet_sky_sheen(float3 normal, float3 v) {
    float3 above = v + float3(0.0, 1.0, 0.0);
    float3 key = dot(above, above) > 1e-6 ? normalize(above) : v;
    return 0.6 * pow(clamp(dot(normal, normalize(v + key)), 0.0, 1.0), 40.0);
}

float3 rikka_wet_film_normal(float3 shading, float3 geometric, RikkaWet w) {
    return normalize(lerp(shading, geometric, w.film));
}

float rikka_wet_film_light(float3 normal, float3 v, float3 l) {
    float3 h = normalize(l + v);
    float n_dot_h = max(dot(normal, h), 0.0);
    float l_dot_h = max(dot(l, h), 1e-3);
    float n_dot_l = max(dot(normal, l), 0.0);
    return rikka_ggx_distribution(n_dot_h, RIKKA_WET_FILM_ROUGHNESS) * (0.25 / (l_dot_h * l_dot_h))
        * rikka_wet_film_fresnel(l_dot_h) * n_dot_l;
}
