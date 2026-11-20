__exported import rikka.pbr.brdf;

struct RikkaSheenData {
    float roughness;
};

RikkaSheenData rikka_closure_sheen_data() {
    RikkaSheenData data;
    data.roughness = 0.5;
    return data;
}

struct RikkaSheenState {
    float3 normal;
    float3 view;
    float n_dot_v;
    float roughness;
    float3 albedo;
    float occlusion;
    float3 emission;
    float alpha;
    float3 lit;
};

struct RikkaSheenModel : IShadingModel {
    typealias Data = RikkaSheenData;
    typealias State = RikkaSheenState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.normal = normalize(surface.normal);
        s.view = pixel.view;
        s.n_dot_v = clamp(dot(s.normal, s.view), 0.0, 1.0);
        s.roughness = clamp(surface.model.roughness, 0.0, 1.0);
        s.albedo = clamp(surface.albedo, float3(0.0), float3(1.0)) * rikka_pbr_sheen_albedo(s.n_dot_v, s.roughness);
        s.occlusion = surface.occlusion;
        s.emission = surface.emission;
        s.alpha = surface.alpha;
        s.lit = float3(0.0);
        return s;
    }

    [constref]
    void direct(inout State s, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        if (light.area()) {
            if (light.window <= 0.0) {
                return;
            }
            float3 visible = shadow.transmittance();
            float lobe = light.specular(context.frame, rikka_pbr_sheen_ltc(s.n_dot_v, s.roughness), s.normal, s.view);
            s.lit += light.radiance() * visible * s.albedo * lobe;
            return;
        }
        if (light.attenuation <= 0.0 || dot(s.normal, light.direction) <= 0.0) {
            return;
        }
        float3 visible = shadow.transmittance();
        float lobe = rikka_pbr_sheen_lobe(s.normal, s.view, light.direction, s.roughness);
        s.lit += light.radiance() * light.attenuation * visible * s.albedo * lobe;
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
        s.lit += s.albedo * indirect.diffuse(s.normal, s.albedo, s.occlusion);
        s.lit += s.emission;
    }

    [constref]
    float4 resolve(State s, RikkaPixel pixel, RikkaContext context) {
        return float4(s.lit, rikka_coverage(context, s.alpha));
    }
};
