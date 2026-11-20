__exported import rikka.pbr.brdf;
import rikka.npr.light;

struct RikkaToonBsdfData {
    float size;
    float smoothness;
    float component;
};

RikkaToonBsdfData rikka_closure_toon_data() {
    RikkaToonBsdfData data;
    data.size = 0.5;
    data.smoothness = 0.0;
    data.component = 0.0;
    return data;
}

struct RikkaToonBsdfState {
    float3 normal;
    float3 view;
    float3 reflected;
    float3 color;
    float edge;
    float smoothness;
    float scale;
    bool glossy;
    float occlusion;
    float3 emission;
    float alpha;
    float3 lit;
};

float rikka_toon_bsdf_intensity(float edge, float smoothness, float angle) {
    if (angle < edge) {
        return 1.0;
    }
    if (angle < edge + smoothness && smoothness > 0.0) {
        return 1.0 - (angle - edge) / smoothness;
    }
    return 0.0;
}

struct RikkaToonBsdfModel : IShadingModel {
    typealias Data = RikkaToonBsdfData;
    typealias State = RikkaToonBsdfState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.normal = normalize(surface.normal);
        s.view = pixel.view;
        s.reflected = reflect(-s.view, s.normal);
        s.color = clamp(surface.albedo, float3(0.0), float3(1.0));
        s.edge = clamp(surface.model.size, 0.0, 1.0) * RIKKA_PI * 0.5;
        s.smoothness = clamp(surface.model.smoothness, 0.0, 1.0) * RIKKA_PI * 0.5;
        float widest = min(s.edge + s.smoothness, RIKKA_PI * 0.5);
        s.scale = 0.5 * RIKKA_INV_PI / max(1.0 - cos(widest), 1e-4);
        s.glossy = surface.model.component > 0.5;
        s.occlusion = surface.occlusion;
        s.emission = surface.emission;
        s.alpha = surface.alpha;
        s.lit = float3(0.0);
        return s;
    }

    [constref]
    void direct(inout State s, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        float3 l = light.direction;
        float n_dot_l = dot(s.normal, l);
        if (light.attenuation <= 0.0 || n_dot_l <= 0.0) {
            return;
        }
        float axis = s.glossy ? dot(s.reflected, l) : n_dot_l;
        float angle = acos(clamp(axis, 0.0, 1.0));
        float spread = asin(rikka_npr_angular_radius(light));
        float intensity = rikka_toon_bsdf_intensity(s.edge, s.smoothness + spread, angle);
        if (intensity <= 0.0) {
            return;
        }
        float reach = light.area() ? light.irradiance(context.frame, s.normal) / max(n_dot_l, 1e-4) : light.attenuation;
        s.lit += light.radiance() * reach * shadow.transmittance() * s.color * (intensity * s.scale);
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
        if (s.glossy) {
            float roughness = clamp(sqrt(s.edge / (RIKKA_PI * 0.5)), 0.03, 1.0);
            s.lit += s.color * indirect.specular(s.reflected, roughness)
                * indirect.specular_occlusion(s.normal, s.view, roughness, s.occlusion);
        } else {
            s.lit += s.color * indirect.diffuse(s.normal, s.color, s.occlusion);
        }
        s.lit += s.emission;
    }

    [constref]
    float4 resolve(State s, RikkaPixel pixel, RikkaContext context) {
        return float4(s.lit, rikka_coverage(context, s.alpha));
    }
};
