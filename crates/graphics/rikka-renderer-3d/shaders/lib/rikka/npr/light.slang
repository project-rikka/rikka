__exported import rikka.npr.head;

struct RikkaNprLight {
    float3 direction;
    float3 color;
    float attenuation;
    float irradiance;
    float leading;
    float area;
    float angular_radius;
    float distance;
};

float rikka_npr_light_distance(RikkaLightSample light) {
    return light.light.kind == RIKKA_LIGHT_DIRECTIONAL ? 0.0 : length(light.light.position - light.position);
}

float rikka_npr_angular_radius(RikkaLightSample light) {
    if (light.area()) {
        return sqrt(clamp(light.vector_form_factor.w, 0.0, 1.0));
    }
    if (light.light.kind == RIKKA_LIGHT_DIRECTIONAL) {
        return sin(clamp(light.light.size, 0.0, RIKKA_PI * 0.5));
    }
    return clamp(light.light.size / max(rikka_npr_light_distance(light), 1e-4), 0.0, 1.0);
}

RikkaNprLight rikka_npr_light(RikkaLightSample light, RikkaContext context, float3 normal) {
    RikkaNprLight l;
    l.direction = light.direction;
    l.color = light.radiance();
    l.attenuation = light.attenuation;
    l.irradiance = light.irradiance(context.frame, normal);
    l.leading = light.leading ? 1.0 : 0.0;
    l.area = light.area() ? 1.0 : 0.0;
    l.angular_radius = rikka_npr_angular_radius(light);
    l.distance = rikka_npr_light_distance(light);
    return l;
}

struct RikkaNprShadow {
    float visibility;
    float3 transmittance;
    float3 tint;
    float3 filter;
    float map;
    float screen;
    float hard;
    float blocker;
    float penumbra;
    float faded;
};

RikkaNprShadow rikka_npr_shadow(inout RikkaShadowQuery shadow, float near) {
    RikkaShadowSample raw = shadow.sample();
    RikkaNprShadow s;
    s.map = raw.visibility;
    s.screen = shadow.screen();
    s.visibility = s.map * s.screen;
    s.transmittance = raw.tint * s.visibility;
    float brightest = max(max(s.transmittance.x, s.transmittance.y), s.transmittance.z);
    s.tint = brightest > 1e-4 ? s.transmittance / brightest : float3(1.0);
    s.filter = raw.tint;
    s.hard = shadow.hard();
    bool measured = raw.blocked && raw.blocker > 0.0;
    s.blocker = measured ? raw.blocker : -1.0;
    s.penumbra = raw.penumbra;
    float fade = measured && near > 0.0 ? smoothstep(0.5, 1.0, raw.blocker / near) : 1.0;
    s.faded = lerp(1.0, s.visibility, fade);
    return s;
}

float rikka_npr_light_scale(RikkaContext context) {
    return context.looked ? RIKKA_INV_PI : 1.0;
}

float rikka_npr_cast_gate(float lit, float shadow, float floor_value, float mask, float bypass, float facing, float terminator) {
    float reach = terminator > 0.0 ? smoothstep(0.0, terminator, facing) : 1.0;
    float cast = mask > bypass ? 1.0 : lerp(1.0, shadow, reach);
    return lerp(floor_value, lit, cast);
}

struct RikkaNprIndirect {
    float3 diffuse;
    float3 sky;
    float3 reflection;
    float occlusion;
    float screen_shadow;
    float3 bent_normal;
};

RikkaNprIndirect rikka_npr_indirect(RikkaIndirect indirect, float3 normal, float3 view, float roughness, float3 albedo, float occlusion) {
    RikkaNprIndirect i;
    float rough = clamp(roughness, 0.03, 1.0);
    i.diffuse = indirect.diffuse(normal, albedo, occlusion);
    i.sky = indirect.irradiance(normal);
    i.reflection = indirect.specular(rikka_pbr_dominant(normal, view, rough), rough)
        * indirect.specular_occlusion(normal, view, rough, occlusion);
    i.occlusion = indirect.occlusion.x;
    i.screen_shadow = indirect.occlusion.y;
    i.bent_normal = indirect.bent(normal);
    return i;
}

struct RikkaNprEnvironment {
    float3 multiply;
    float3 add;
    float3 equalized;
};

RikkaNprEnvironment rikka_npr_environment(RikkaIndirect indirect, float3 normal, float3 main_light, float strength, float equalization, float lowest) {
    float3 up = indirect.irradiance(float3(0.0, 1.0, 0.0));
    float3 down = indirect.irradiance(float3(0.0, -1.0, 0.0));
    float3 average = 0.5 * (up + down);
    RikkaNprEnvironment e;
    e.multiply = max(max(float3(lowest), max(average, down)), main_light);
    float3 gi = min(indirect.irradiance(normal), float3(1.0));
    float level = rikka_luma(gi);
    e.add = gi * level * strength * (1.0 - smoothstep(0.0, 1.0, level * 0.5));
    e.equalized = lerp(indirect.irradiance(normal), average, clamp(equalization, 0.0, 1.0));
    return e;
}
