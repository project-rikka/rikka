__exported import rikka.pbr.brdf;

struct RikkaDiffuseData {
    float roughness;
};

RikkaDiffuseData rikka_closure_diffuse_data() {
    RikkaDiffuseData data;
    data.roughness = 0.0;
    return data;
}

struct RikkaDiffuseState {
    float3 normal;
    float3 view;
    float n_dot_v;
    float3 rho;
    float roughness;
    float occlusion;
    float3 emission;
    float alpha;
    float3 lit;
};

struct RikkaDiffuseModel : IShadingModel {
    typealias Data = RikkaDiffuseData;
    typealias State = RikkaDiffuseState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.normal = normalize(surface.normal);
        s.view = pixel.view;
        s.n_dot_v = max(dot(s.normal, s.view), RIKKA_PBR_EPSILON);
        s.rho = clamp(surface.albedo, float3(0.0), float3(1.0));
        s.roughness = clamp(surface.model.roughness, 0.0, 1.0);
        s.occlusion = surface.occlusion;
        s.emission = surface.emission;
        s.alpha = surface.alpha;
        s.lit = float3(0.0);
        return s;
    }

    [constref]
    void direct(inout State s, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        if (light.area()) {
            float irradiance = light.irradiance(context.frame, s.normal);
            if (irradiance <= 0.0) {
                return;
            }
            float3 visible = shadow.transmittance();
            s.lit += light.radiance() * visible * rikka_pbr_eon_albedo(s.rho, s.roughness, s.n_dot_v)
                * (irradiance * RIKKA_INV_PI);
            return;
        }
        float n_dot_l = dot(s.normal, light.direction);
        if (light.attenuation <= 0.0 || n_dot_l <= 0.0) {
            return;
        }
        float3 visible = shadow.transmittance();
        RikkaPbrBasis basis = rikka_pbr_basis(s.normal, s.view);
        float3 f = rikka_pbr_eon(s.rho, s.roughness, basis.local(light.direction), basis.local(s.view));
        s.lit += light.radiance() * light.attenuation * visible * f * n_dot_l;
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
        s.lit += indirect.diffuse(s.normal, s.rho, s.occlusion) * rikka_pbr_eon_albedo(s.rho, s.roughness, s.n_dot_v);
        s.lit += s.emission;
    }

    [constref]
    float4 resolve(State s, RikkaPixel pixel, RikkaContext context) {
        return float4(s.lit, rikka_coverage(context, s.alpha));
    }
};
