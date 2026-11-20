__exported import rikka.material;

struct RikkaStandardData {
    float metallic;
    float roughness;
};

struct RikkaStandardState {
    float3 normal;
    float3 geometric_normal;
    float3 view;
    float n_dot_v;
    RikkaWet wet;
    float3 albedo;
    float3 film_normal;
    float film_view;
    float beneath;
    float3 f0;
    float3 diffuse_albedo;
    float roughness;
    float2 brdf;
    float3 energy;
    float occlusion;
    float3 emission;
    float alpha;
    float3 lit;
};

struct RikkaStandardModel : IShadingModel {
    RikkaGloss gloss;

    typealias Data = RikkaStandardData;
    typealias State = RikkaStandardState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.normal = normalize(surface.normal);
        s.geometric_normal = pixel.geometric_normal;
        s.view = pixel.view;
        s.n_dot_v = max(dot(s.normal, s.view), 1e-4);
        s.wet = rikka_wet(pixel.wet, rikka_wet_porosity(surface.model.roughness, surface.model.metallic));
        s.albedo = rikka_wet_albedo(surface.albedo, s.wet);
        s.film_normal = rikka_wet_film_normal(s.normal, pixel.geometric_normal, s.wet);
        s.film_view = s.wet.film * rikka_wet_film_fresnel(max(dot(s.film_normal, s.view), 1e-4));
        s.beneath = 1.0 - s.film_view;
        s.f0 = lerp(float3(0.04), s.albedo, surface.model.metallic);
        s.diffuse_albedo = s.albedo * (1.0 - surface.model.metallic);
        s.roughness = clamp(rikka_wet_roughness(surface.model.roughness, s.wet), 0.03, 1.0);
        s.brdf = rikka_environment_brdf(s.n_dot_v, s.roughness);
        s.energy = context.looked
            ? float3(1.0) + s.f0 * (1.0 / max(s.brdf.x + s.brdf.y, 1e-3) - 1.0)
            : float3(1.0);
        s.occlusion = surface.occlusion;
        s.emission = surface.emission;
        s.alpha = surface.alpha;
        s.lit = float3(0.0);
        return s;
    }

    [constref]
    void direct(inout State s, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        if (light.area()) {
            area(s, light, shadow, context);
            return;
        }
        float3 l = light.direction;
        float n_dot_l = dot(s.normal, l);
        if (light.attenuation <= 0.0 || n_dot_l <= 0.0) {
            return;
        }
        float3 visible = shadow.transmittance();
        if (all(visible <= float3(0.0))) {
            return;
        }
        float3 h = normalize(l + s.view);
        float n_dot_h = max(dot(s.normal, h), 0.0);
        float v_dot_h = max(dot(s.view, h), 0.0);

        float3 fresnel = rikka_fresnel(s.f0, v_dot_h);
        float distribution = rikka_ggx_distribution(n_dot_h, s.roughness);
        float visibility = rikka_smith_visibility(s.n_dot_v, n_dot_l, s.roughness);

        float3 specular = fresnel * distribution * visibility * s.energy;
        float3 diffuse = (float3(1.0) - fresnel) * s.diffuse_albedo * (1.0 / 3.14159265);
        float3 incident = light.radiance() * light.attenuation * visible;
        s.lit += (diffuse + specular) * incident * n_dot_l * s.beneath;
        if (s.wet.film > 0.0) {
            s.lit += incident * s.wet.film * rikka_wet_film_light(s.film_normal, s.view, l);
        }
    }

    [constref]
    void area(inout State s, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        float irradiance = light.irradiance(context.frame, s.normal);
        if (irradiance <= 0.0 && light.window <= 0.0) {
            return;
        }
        float3 visible = shadow.transmittance();
        if (all(visible <= float3(0.0))) {
            return;
        }
        RikkaLtc ltc = rikka_ltc(context.frame, s.roughness, s.n_dot_v);
        float3 albedo = s.f0 * ltc.magnitude + (float3(1.0) - s.f0) * ltc.fresnel;
        float specular = light.specular(context.frame, ltc, s.normal, s.view);
        float3 radiance = light.radiance() * visible;
        float3 diffuse = (float3(1.0) - albedo) * s.diffuse_albedo * (irradiance * (1.0 / 3.14159265));
        s.lit += (diffuse + albedo * s.energy * specular) * radiance * s.beneath;
        if (s.wet.film > 0.0) {
            float3 l = light.direction;
            s.lit += radiance * light.attenuation * s.wet.film
                * rikka_wet_film_light(s.film_normal, s.view, l);
        }
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
        float3 film_reflection = s.wet.film > 0.0
            ? indirect.specular(reflect(-s.view, s.film_normal), RIKKA_WET_FILM_ROUGHNESS) * s.film_view
            : float3(0.0);

        if (!indirect.looked) {
            s.lit += indirect.ambient * s.albedo * s.occlusion * s.beneath;
            s.lit = s.lit + film_reflection * s.occlusion + s.emission;
            return;
        }

        float3 reflected = reflect(-s.view, s.normal);
        float a2 = s.roughness * s.roughness * s.roughness * s.roughness;
        float3 dominant = normalize(lerp(reflected, s.normal, a2));
        float horizon = clamp(1.0 + 1.1 * dot(reflected, s.geometric_normal), 0.0, 1.0);
        float3 specular_color = s.f0 * s.brdf.x + s.brdf.y;
        float3 irradiance = indirect.diffuse(s.normal, s.diffuse_albedo, s.occlusion);
        float3 radiance = indirect.specular(dominant, s.roughness);
        s.lit += irradiance * s.diffuse_albedo * (float3(1.0) - specular_color) * s.beneath;
        s.lit += radiance * specular_color * s.energy * s.beneath
            * indirect.specular_occlusion(s.normal, s.view, s.roughness, s.occlusion) * horizon * horizon;
        s.lit += film_reflection * indirect.specular_occlusion(s.film_normal, s.view, RIKKA_WET_FILM_ROUGHNESS, s.occlusion);
        s.lit = s.lit + s.emission;
    }

    [constref]
    float4 resolve(State s, RikkaPixel pixel, RikkaContext context) {
        return float4(s.lit, rikka_coverage(context, s.alpha));
    }

    [constref]
    override RikkaGBuffer gbuffer(RikkaContext context) {
        return rikka_gbuffer(context, gloss);
    }
};

struct RikkaUvTransform {
    [Default(0.0, 0.0)] [Group("uv")] float2 offset;
    [Default(1.0, 1.0)] [Group("uv")] float2 scale;
    [Default(0.0)] [Group("uv")] [Hint("angle")] float rotation;

    [constref]
    float2 apply(float2 uv) {
        return rikka_uv_transform(uv, offset, scale, rotation);
    }
};
