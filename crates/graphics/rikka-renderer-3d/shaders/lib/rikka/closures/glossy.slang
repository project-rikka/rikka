__exported import rikka.pbr.openpbr;

struct RikkaGlossyData {
    float roughness;
    float anisotropy;
    float3 tangent;
};

RikkaGlossyData rikka_closure_glossy_data() {
    RikkaGlossyData data;
    data.roughness = 0.3;
    data.anisotropy = 0.0;
    data.tangent = float3(0.0);
    return data;
}

struct RikkaGlossyState {
    RikkaPbrBasis basis;
    float3 geometric_normal;
    float3 view;
    float3 v;
    float n_dot_v;
    float roughness;
    float anisotropy;
    float2 alpha;
    float3 albedo;
    float3 reflectance;
    float occlusion;
    float3 emission;
    float alpha_coverage;
    float3 lit;
};

struct RikkaGlossyModel : IShadingModel {
    typealias Data = RikkaGlossyData;
    typealias State = RikkaGlossyState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        float3 n = normalize(surface.normal);
        float3 tangent = dot(surface.model.tangent, surface.model.tangent) > 1e-8 ? surface.model.tangent : pixel.tangent.xyz;
        s.basis = rikka_pbr_basis(n, tangent);
        s.geometric_normal = pixel.geometric_normal;
        s.view = pixel.view;
        s.v = s.basis.local(s.view);
        s.v.z = max(s.v.z, RIKKA_PBR_EPSILON);
        s.n_dot_v = s.v.z;
        s.roughness = clamp(surface.model.roughness, 0.0, 1.0);
        s.anisotropy = clamp(surface.model.anisotropy, 0.0, 1.0);
        s.alpha = rikka_pbr_filtered_alpha(rikka_pbr_alpha(s.roughness, s.anisotropy), rikka_pbr_filter_kernel(n));
        float3 color = clamp(surface.albedo, float3(0.0), float3(1.0));
        float2 dfg = rikka_pbr_dfg(context.frame, s.n_dot_v, s.roughness);
        float3 energy = rikka_pbr_energy(color, dfg);
        s.reflectance = color * energy;
        s.albedo = s.reflectance * (dfg.x + dfg.y);
        s.occlusion = surface.occlusion;
        s.emission = surface.emission;
        s.alpha_coverage = surface.alpha;
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
            RikkaLtc ltc = rikka_ltc(context.frame, rikka_openpbr_ltc_roughness(s.alpha), s.n_dot_v);
            float lobe = light.specular(context.frame, ltc, s.basis.n, s.view);
            s.lit += light.radiance() * visible * s.albedo * lobe;
            return;
        }
        float3 l = s.basis.local(light.direction);
        if (light.attenuation <= 0.0 || l.z <= 0.0) {
            return;
        }
        float3 visible = shadow.transmittance();
        float3 h = normalize(s.v + l);
        float lobe = rikka_pbr_ggx(h, s.alpha) * rikka_pbr_visibility(s.v, l, s.alpha) * l.z;
        s.lit += light.radiance() * light.attenuation * visible * s.reflectance * lobe;
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
        float3 n = s.basis.n;
        float3 reflected = reflect(-s.view, n);
        float3 direction = s.anisotropy > 0.0
            ? rikka_pbr_bent_reflection(s.basis, s.view, s.roughness, s.anisotropy)
            : rikka_pbr_dominant(n, s.view, s.roughness);
        float occlusion = indirect.specular_occlusion(n, s.view, s.roughness, s.occlusion)
            * rikka_pbr_horizon(reflected, s.geometric_normal);
        s.lit += s.albedo * indirect.specular(direction, s.roughness) * occlusion;
        s.lit += s.emission;
    }

    [constref]
    float4 resolve(State s, RikkaPixel pixel, RikkaContext context) {
        return float4(s.lit, rikka_coverage(context, s.alpha_coverage));
    }
};
