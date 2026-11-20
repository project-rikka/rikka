__exported import rikka.pbr.brdf;
__exported import rikka.standard;

struct RikkaEyeData {
    float3 forward;
    float iris;
    float cornea_roughness;
    float cornea_ior;
    float caustic;
    float sclera_scatter;
};

struct RikkaEyeState {
    float3 normal;
    float3 forward;
    float3 iris_normal;
    float3 geometric_normal;
    float3 view;
    float n_dot_v;
    float3 albedo;
    float iris;
    float2 alpha;
    float f0;
    float ior;
    float caustic;
    float sclera_scatter;
    float3 energy;
    float specular_albedo;
    float occlusion;
    float3 emission;
    float alpha_value;
    float3 lit;
};

struct RikkaEyeModel : IShadingModel {
    RikkaGloss gloss;

    typealias Data = RikkaEyeData;
    typealias State = RikkaEyeState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.normal = normalize(surface.normal);
        s.forward = normalize(surface.model.forward);
        s.iris_normal = normalize(2.0 * dot(s.normal, s.forward) * s.forward - s.normal);
        s.geometric_normal = pixel.geometric_normal;
        s.view = pixel.view;
        s.n_dot_v = max(dot(s.normal, s.view), RIKKA_PBR_EPSILON);
        s.albedo = clamp(surface.albedo, float3(0.0), float3(1.0));
        s.iris = clamp(surface.model.iris, 0.0, 1.0);
        float kernel = rikka_pbr_filter_kernel(s.normal);
        s.alpha = rikka_pbr_filtered_alpha(rikka_pbr_alpha(surface.model.cornea_roughness, 0.0), kernel);
        s.ior = max(surface.model.cornea_ior, 1.0);
        s.f0 = rikka_pbr_f0(s.ior);
        s.caustic = max(surface.model.caustic, 0.0);
        s.sclera_scatter = clamp(surface.model.sclera_scatter, 0.0, 1.0);
        float roughness = sqrt(s.alpha.x);
        float2 dfg = rikka_pbr_dfg(context.frame, s.n_dot_v, roughness);
        s.energy = rikka_pbr_energy(float3(s.f0), dfg);
        s.specular_albedo = (s.f0 * dfg.x + dfg.y) * s.energy.x;
        s.occlusion = surface.occlusion;
        s.emission = surface.emission;
        s.alpha_value = surface.alpha;
        s.lit = float3(0.0);
        return s;
    }

    [constref]
    float3 diffuse(State s, float3 l) {
        float3 bent = refract(-l, s.normal, 1.0 / s.ior);
        float through = 1.0 - rikka_pbr_schlick(s.f0, clamp(dot(s.normal, l), 0.0, 1.0));
        float focus = clamp(dot(-bent, s.iris_normal), 0.0, 1.0);
        float iris = focus * through * (1.0 + s.caustic * focus * focus * focus * focus);
        float wrap = s.sclera_scatter * 0.5;
        float sclera = clamp((dot(s.normal, l) + wrap) / (1.0 + wrap), 0.0, 1.0) * through;
        return s.albedo * lerp(sclera, iris, s.iris) / RIKKA_PI;
    }

    [constref]
    void direct(inout State s, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        if (light.attenuation <= 0.0) {
            return;
        }
        float visible = shadow.visibility();
        if (visible <= 0.0) {
            return;
        }
        float3 incident = light.light.color * light.light.intensity * light.attenuation * visible;
        float3 l = light.direction;
        float n_dot_l = dot(s.normal, l);
        float3 lit = diffuse(s, l);
        if (n_dot_l > 0.0) {
            RikkaPbrBasis basis = rikka_pbr_basis(s.normal, s.forward);
            float3 lv = basis.local(l);
            float3 vv = basis.local(s.view);
            vv.z = max(vv.z, RIKKA_PBR_EPSILON);
            float3 h = normalize(lv + vv);
            lit += rikka_pbr_schlick(s.f0, clamp(dot(vv, h), 0.0, 1.0)) * s.energy
                * rikka_pbr_ggx(h, s.alpha) * rikka_pbr_visibility(vv, lv, s.alpha) * n_dot_l;
        }
        s.lit += incident * lit;
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
        float roughness = sqrt(s.alpha.x);
        float3 reflected = reflect(-s.view, s.normal);
        float3 radiance = indirect.specular(reflected, roughness)
            * indirect.specular_occlusion(s.normal, s.view, roughness, s.occlusion)
            * rikka_pbr_horizon(reflected, s.geometric_normal);
        float3 irradiance = lerp(indirect.diffuse(s.normal, s.albedo, s.occlusion),
            indirect.diffuse(s.iris_normal, s.albedo, s.occlusion), s.iris);
        s.lit += irradiance * s.albedo * (1.0 - s.specular_albedo) + radiance * s.specular_albedo + s.emission;
    }

    [constref]
    float4 resolve(State s, RikkaPixel pixel, RikkaContext context) {
        return float4(s.lit, rikka_coverage(context, s.alpha_value));
    }

    [constref]
    override RikkaGBuffer gbuffer(RikkaContext context) {
        return rikka_gbuffer(context, gloss);
    }
};

struct RikkaEyeSurface : ISurface<RikkaEyeData> {
    [Color] [Srgb] [Default(0.9, 0.88, 0.86)] [Group("sclera")] [Label("Color")] [Role("albedo")] float3 sclera_color;
    [Texture("colour")] [Group("sclera")] [Role("albedo_texture")] RikkaTexture sclera_texture;
    [Default(0.5)] [Range(0.0, 1.0)] [Group("sclera")] [Label("Scatter")] float sclera_scatter;
    [Color] [Srgb] [Default(0.25, 0.4, 0.55)] [Group("iris")] [Label("Color")]
    [Preset("brown", 0.35, 0.2, 0.1)] [Preset("green", 0.25, 0.42, 0.22)] [Preset("blue", 0.25, 0.4, 0.55)] [Preset("grey", 0.45, 0.48, 0.5)]
    float3 iris_color;
    [Texture("colour")] [Group("iris")] RikkaTexture iris_texture;
    [Default(0.5, 0.5)] [Group("iris")] [Label("Centre")] float2 iris_center;
    [Default(0.15, 0.15)] [Group("iris")] [Label("Radius")] float2 iris_radius;
    [Default(0.35)] [Range(0.0, 1.0)] [Group("iris")] [Label("Pupil")] float pupil;
    [Default(0.06)] [Range(0.0, 0.5)] [Group("iris")] [Label("Edge softness")] float iris_softness;
    [Default(0.35)] [Range(0.0, 1.0)] [Group("iris")] [Label("Depth")] float iris_depth;
    [Default(0.12)] [Range(0.0, 0.5)] [Group("iris")] [Label("Limbal ring width")] float limbal_width;
    [Default(0.6)] [Range(0.0, 1.0)] [Group("iris")] [Label("Limbal ring darkness")] float limbal_darkness;
    [Default(2.0)] [SoftRange(0.0, 8.0)] [Group("iris")] [Label("Caustic")] float caustic;
    [Default(1.376)] [SoftRange(1.0, 2.0)] [Group("cornea")] [Label("IOR")] float cornea_ior;
    [Default(0.04)] [Range(0.0, 1.0)] [Group("cornea")] [Label("Roughness")] float cornea_roughness;
    [Texture("normal")] [Group("cornea")] RikkaTexture normal_texture;
    [Default(1.0)] [SoftRange(0.0, 2.0)] [Group("cornea")] float normal_scale;
    [Default(0.5)] [Range(0.0, 1.0)] [Group("cornea")] [Label("Bulge")] float cornea_bulge;
    RikkaUvTransform uv;

    [constref]
    RikkaSurfaceData<RikkaEyeData> evaluate(RikkaPixel pixel, RikkaContext context) {
        float2 at = uv.apply(pixel.uv);
        float3 n = pixel.geometric_normal;
        float3 t = normalize(pixel.tangent.xyz - n * dot(n, pixel.tangent.xyz));
        float3 b = cross(n, t) * pixel.tangent.w;
        float2 offset = (at - iris_center) / max(iris_radius, float2(1e-4));
        float distance = length(offset);
        float mask = 1.0 - smoothstep(1.0 - iris_softness, 1.0, distance);
        float3 forward = normalize(n - (offset.x * t + offset.y * b) * 0.45 * mask);

        float3 cornea = n;
        if (normal_texture.bound()) {
            float3 tangent_normal = rikka_unpack_normal(normal_texture.sampled(at));
            tangent_normal.xy *= normal_scale;
            cornea = pixel.normal_map(normalize(tangent_normal));
        }
        float bulge = cornea_bulge * mask * (1.0 - distance * distance);
        cornea = normalize(lerp(cornea, normalize(cornea + forward * bulge), mask));

        float3 bent = refract(-pixel.view, cornea, 1.0 / max(cornea_ior, 1.0));
        float facing = max(dot(-bent, forward), 0.15);
        float2 sideways = float2(dot(bent, t), dot(bent, b)) / facing;
        float height = iris_depth * max(1.0 - distance * distance, 0.0);
        float2 shifted = offset + sideways * height;
        float radial = length(shifted);
        float2 iris_uv = iris_center + shifted * iris_radius;

        float3 iris = iris_color * iris_texture.sampled(iris_uv).rgb;
        float pupil_edge = 1.0 - smoothstep(pupil - 0.04, pupil + 0.04, radial);
        iris = lerp(iris, float3(0.01), pupil_edge);
        float limbal = smoothstep(1.0 - limbal_width, 1.0, radial);
        iris *= 1.0 - limbal_darkness * limbal;
        float3 sclera = sclera_color * sclera_texture.sampled(at).rgb;

        RikkaEyeData d;
        d.forward = forward;
        d.iris = mask;
        d.cornea_roughness = cornea_roughness;
        d.cornea_ior = cornea_ior;
        d.caustic = caustic;
        d.sclera_scatter = sclera_scatter;
        RikkaSurfaceData<RikkaEyeData> s = rikka_surface_data(pixel, d);
        s.normal = cornea;
        s.albedo = lerp(sclera, iris, mask);
        return s;
    }
};
