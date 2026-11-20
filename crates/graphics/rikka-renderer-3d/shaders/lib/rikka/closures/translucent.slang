__exported import rikka.pbr.brdf;

struct RikkaTranslucentData {
    uint unused;
};

RikkaTranslucentData rikka_closure_translucent_data() {
    RikkaTranslucentData data;
    data.unused = 0u;
    return data;
}

struct RikkaTranslucentState {
    float3 normal;
    float3 color;
    float occlusion;
    float3 emission;
    float alpha;
    float3 lit;
};

struct RikkaTranslucentModel : IShadingModel {
    typealias Data = RikkaTranslucentData;
    typealias State = RikkaTranslucentState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.normal = normalize(surface.normal);
        s.color = clamp(surface.albedo, float3(0.0), float3(1.0));
        s.occlusion = surface.occlusion;
        s.emission = surface.emission;
        s.alpha = surface.alpha;
        s.lit = float3(0.0);
        return s;
    }

    [constref]
    void direct(inout State s, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        float behind = light.area()
            ? light.irradiance(context.frame, -s.normal)
            : light.attenuation * max(-dot(s.normal, light.direction), 0.0);
        if (behind <= 0.0) {
            return;
        }
        s.lit += light.radiance() * shadow.transmittance() * s.color * (behind * RIKKA_INV_PI);
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
        s.lit += s.color * indirect.irradiance(-s.normal) * indirect.occlusion.x * s.occlusion;
        s.lit += s.emission;
    }

    [constref]
    float4 resolve(State s, RikkaPixel pixel, RikkaContext context) {
        return float4(s.lit, rikka_coverage(context, s.alpha));
    }
};
