__exported import rikka.pbr.brdf;
__exported import rikka.standard;
__exported import rikka.surface;

static const float RIKKA_OPENPBR_REFRACTION_TAPS = 8.0;
static const float RIKKA_OPENPBR_REFRACTION_BLUR = 48.0;
static const float RIKKA_OPENPBR_FRAUNHOFER_C = 656.3;
static const float RIKKA_OPENPBR_FRAUNHOFER_D = 587.6;
static const float RIKKA_OPENPBR_FRAUNHOFER_F = 486.1;
static const float3 RIKKA_OPENPBR_WAVELENGTHS = float3(630.0, 532.0, 465.0);

struct RikkaOpenPbrData {
    float metalness;
    float diffuse_roughness;
    float specular_weight;
    float3 specular_color;
    float specular_roughness;
    float specular_anisotropy;
    float specular_ior;
    float3 tangent;
    float transmission_weight;
    float3 transmission_color;
    float transmission_depth;
    float dispersion;
    float thickness;
    float subsurface_weight;
    float3 subsurface_color;
    float3 subsurface_radius;
    float subsurface_anisotropy;
    float coat_weight;
    float3 coat_color;
    float coat_roughness;
    float coat_anisotropy;
    float coat_ior;
    float coat_darkening;
    float3 coat_normal;
    float fuzz_weight;
    float3 fuzz_color;
    float fuzz_roughness;
    float thin_film_weight;
    float thin_film_thickness;
    float thin_film_ior;
    bool thin_walled;
    float second_lobe;
    float second_roughness;
    float cavity;
    bool unscattered;
};

RikkaOpenPbrData rikka_openpbr_data() {
    RikkaOpenPbrData d;
    d.metalness = 0.0;
    d.diffuse_roughness = 0.0;
    d.specular_weight = 1.0;
    d.specular_color = float3(1.0);
    d.specular_roughness = 0.3;
    d.specular_anisotropy = 0.0;
    d.specular_ior = 1.5;
    d.tangent = float3(0.0);
    d.transmission_weight = 0.0;
    d.transmission_color = float3(1.0);
    d.transmission_depth = 0.0;
    d.dispersion = 0.0;
    d.thickness = 0.0;
    d.subsurface_weight = 0.0;
    d.subsurface_color = float3(0.8);
    d.subsurface_radius = float3(1.0, 0.5, 0.25);
    d.subsurface_anisotropy = 0.0;
    d.coat_weight = 0.0;
    d.coat_color = float3(1.0);
    d.coat_roughness = 0.0;
    d.coat_anisotropy = 0.0;
    d.coat_ior = 1.6;
    d.coat_darkening = 1.0;
    d.coat_normal = float3(0.0);
    d.fuzz_weight = 0.0;
    d.fuzz_color = float3(1.0);
    d.fuzz_roughness = 0.5;
    d.thin_film_weight = 0.0;
    d.thin_film_thickness = 0.5;
    d.thin_film_ior = 1.4;
    d.thin_walled = false;
    d.second_lobe = 0.0;
    d.second_roughness = 0.3;
    d.cavity = 1.0;
    d.unscattered = false;
    return d;
}

float3 rikka_openpbr_burley_shape(float3 albedo) {
    float3 a = clamp(albedo, float3(0.0), float3(1.0));
    float3 offset = abs(a - 0.8);
    return 1.85 - a + 7.0 * offset * offset * offset;
}

struct RikkaOpenPbrDiffuse {
    float3 lit;
    float3 scatter;
};

struct RikkaOpenPbrState {
    RikkaPbrBasis basis;
    float3 coat_normal;
    float3 geometric_normal;
    float3 view;
    float3 v;
    float n_dot_v;
    RikkaWet wet;
    float3 film_normal;
    float film_view;
    float3 rho;
    float diffuse_roughness;
    float2 alpha;
    float2 second_alpha;
    float second_lobe;
    float roughness;
    float anisotropy;
    float3 dielectric_f0;
    float dielectric_f90;
    float3 specular_tint;
    float3 metal_f0;
    float3 metal_tint;
    float specular_weight;
    float metalness;
    float eta;
    float3 dielectric_energy;
    float3 metal_energy;
    float3 dielectric_albedo;
    float3 metal_albedo;
    float3 film;
    float3 film_metal;
    float film_weight;
    float transmission;
    float3 transmission_color;
    float transmission_depth;
    float dispersion;
    float thickness;
    float subsurface;
    float3 subsurface_color;
    float3 subsurface_radius;
    float subsurface_anisotropy;
    bool thin_walled;
    float coat;
    float3 coat_color;
    float2 coat_alpha;
    float coat_roughness;
    float coat_f0;
    float coat_ior;
    float coat_energy;
    float coat_albedo;
    float3 coat_base;
    float3 layer;
    float3 diffuse_weight;
    float3 scatter_weight;
    float fuzz;
    float3 fuzz_color;
    float fuzz_roughness;
    float fuzz_albedo;
    float cavity;
    float occlusion;
    float3 emission;
    float opacity;
    bool scattered;
    float3 lit;
    float3 scatter;
};

float3 rikka_openpbr_coat_transmittance(float3 coat_color, float eta, float mu_i, float mu_o) {
    float inverse = 1.0 / max(eta * eta, 1e-4);
    float refracted_i = sqrt(max(1.0 - (1.0 - mu_i * mu_i) * inverse, 1e-4));
    float refracted_o = sqrt(max(1.0 - (1.0 - mu_o * mu_o) * inverse, 1e-4));
    return pow(max(coat_color, float3(1e-4)), float3(0.5 * (1.0 / refracted_i + 1.0 / refracted_o)));
}

float rikka_openpbr_ltc_roughness(float2 alpha) {
    return sqrt(sqrt(alpha.x * alpha.y));
}

struct RikkaOpenPbrModel : IShadingModel, IScattering {
    RikkaGloss gloss;

    typealias Data = RikkaOpenPbrData;
    typealias State = RikkaOpenPbrState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        RikkaOpenPbrData d = surface.model;
        State s;
        float3 n = normalize(surface.normal);
        s.basis = rikka_pbr_basis(n, dot(d.tangent, d.tangent) > 1e-8 ? d.tangent : pixel.tangent.xyz);
        s.geometric_normal = pixel.geometric_normal;
        s.view = pixel.view;
        s.v = s.basis.local(s.view);
        s.v.z = max(s.v.z, RIKKA_PBR_EPSILON);
        s.n_dot_v = s.v.z;
        s.coat_normal = dot(d.coat_normal, d.coat_normal) > 1e-8 ? normalize(d.coat_normal) : n;

        s.metalness = clamp(d.metalness, 0.0, 1.0);
        float rough = clamp(d.specular_roughness, 0.0, 1.0);
        s.wet = rikka_wet(pixel.wet, rikka_wet_porosity(rough, s.metalness));
        s.rho = clamp(rikka_wet_albedo(surface.albedo, s.wet), float3(0.0), float3(1.0));
        s.film_normal = rikka_wet_film_normal(n, pixel.geometric_normal, s.wet);
        s.film_view = s.wet.film * rikka_wet_film_fresnel(max(dot(s.film_normal, s.view), 1e-4));
        rough = rikka_wet_roughness(rough, s.wet);
        s.diffuse_roughness = clamp(d.diffuse_roughness, 0.0, 1.0);

        s.coat = rikka_feature(RIKKA_PBR_COAT) ? clamp(d.coat_weight, 0.0, 1.0) : 0.0;
        s.coat_color = clamp(d.coat_color, float3(0.0), float3(1.0));
        s.coat_ior = max(d.coat_ior, 1.0);
        s.coat_roughness = clamp(d.coat_roughness, 0.0, 1.0);
        if (s.coat > 0.0) {
            float roughened = pow(min(1.0, pow(rough, 4.0) + 2.0 * pow(s.coat_roughness, 4.0)), 0.25);
            rough = lerp(rough, roughened, s.coat);
        }
        s.roughness = rough;
        s.anisotropy = rikka_feature(RIKKA_PBR_ANISOTROPY) ? clamp(d.specular_anisotropy, 0.0, 1.0) : 0.0;
        float kernel = rikka_pbr_filter_kernel(n);
        s.alpha = rikka_pbr_filtered_alpha(rikka_pbr_alpha(rough, s.anisotropy), kernel);
        s.second_lobe = rikka_feature(RIKKA_PBR_SECOND_LOBE) ? clamp(d.second_lobe, 0.0, 1.0) : 0.0;
        s.second_alpha = s.second_lobe > 0.0
            ? rikka_pbr_filtered_alpha(rikka_pbr_alpha(clamp(d.second_roughness, 0.0, 1.0), s.anisotropy), kernel)
            : s.alpha;
        s.coat_alpha = rikka_pbr_filtered_alpha(rikka_pbr_alpha(s.coat_roughness, clamp(d.coat_anisotropy, 0.0, 1.0)), kernel);

        float base_ior = max(d.specular_ior, 1e-3);
        float coated = s.coat_ior > base_ior ? s.coat_ior / base_ior : base_ior / s.coat_ior;
        float eta = lerp(base_ior, coated, s.coat);
        s.eta = rikka_pbr_modulated_eta(eta, max(d.specular_weight, 0.0));
        s.specular_weight = max(d.specular_weight, 0.0);
        s.specular_tint = clamp(d.specular_color, float3(0.0), float3(1.0));
        float f0 = rikka_pbr_f0(s.eta);
        s.dielectric_f0 = float3(f0);
        s.dielectric_f90 = rikka_pbr_f90(f0);
        s.metal_f0 = s.rho;
        s.metal_tint = s.specular_tint;

        float2 dfg = rikka_pbr_dfg(context.frame, s.n_dot_v, rough);
        s.dielectric_energy = rikka_pbr_energy(s.dielectric_f0, dfg);
        s.metal_energy = rikka_pbr_energy(s.metal_f0, dfg);
        float3 dielectric = s.specular_tint * rikka_pbr_specular_albedo(s.dielectric_f0, float3(s.dielectric_f90), dfg);
        float3 schlick = rikka_pbr_schlick(s.metal_f0, float3(1.0), s.n_dot_v);
        float3 edge = rikka_pbr_f82(s.metal_f0, s.metal_tint, s.n_dot_v) / max(schlick, float3(1e-4));
        float3 metal = s.specular_weight * rikka_pbr_specular_albedo(s.metal_f0, float3(1.0), dfg) * edge;
        s.film_weight = rikka_feature(RIKKA_PBR_FILM) ? clamp(d.thin_film_weight, 0.0, 1.0) : 0.0;
        s.film = float3(1.0);
        s.film_metal = float3(1.0);
        if (s.film_weight > 0.0) {
            float outside = lerp(1.0, s.coat_ior, s.coat);
            float thickness = max(d.thin_film_thickness, 0.0) * 1000.0;
            float3 film_dielectric = rikka_pbr_thin_film(outside, max(d.thin_film_ior, 1.0), s.n_dot_v, thickness, s.dielectric_f0);
            float3 film_metal = rikka_pbr_thin_film(outside, max(d.thin_film_ior, 1.0), s.n_dot_v, thickness, s.metal_f0);
            s.film = film_dielectric;
            s.film_metal = film_metal;
            dielectric = lerp(dielectric, s.specular_tint * film_dielectric * (dfg.x + dfg.y), s.film_weight);
            metal = lerp(metal, s.specular_weight * film_metal * (dfg.x + dfg.y), s.film_weight);
        }
        s.dielectric_albedo = clamp(dielectric * s.dielectric_energy, float3(0.0), float3(1.0));
        s.metal_albedo = clamp(metal * s.metal_energy, float3(0.0), float3(1.0));

        s.transmission = rikka_feature(RIKKA_PBR_TRANSMISSION) ? clamp(d.transmission_weight, 0.0, 1.0) : 0.0;
        s.transmission_color = clamp(d.transmission_color, float3(0.0), float3(1.0));
        s.transmission_depth = max(d.transmission_depth, 0.0);
        s.dispersion = max(d.dispersion, 0.0);
        s.thickness = max(d.thickness, 0.0);
        s.subsurface = rikka_feature(RIKKA_PBR_SUBSURFACE) ? clamp(d.subsurface_weight, 0.0, 1.0) : 0.0;
        s.subsurface_color = clamp(d.subsurface_color, float3(0.0), float3(1.0));
        s.subsurface_radius = max(d.subsurface_radius, float3(0.0));
        s.subsurface_anisotropy = clamp(d.subsurface_anisotropy, -0.95, 0.95);
        s.thin_walled = d.thin_walled;

        s.coat_f0 = rikka_pbr_f0(s.coat_ior);
        s.coat_energy = 1.0;
        s.coat_albedo = 0.0;
        s.coat_base = float3(1.0);
        if (s.coat > 0.0) {
            float coat_n_dot_v = max(dot(s.coat_normal, s.view), RIKKA_PBR_EPSILON);
            float2 coat_dfg = rikka_pbr_dfg(context.frame, coat_n_dot_v, s.coat_roughness);
            s.coat_energy = rikka_pbr_energy(float3(s.coat_f0), coat_dfg).x;
            s.coat_albedo = clamp((s.coat_f0 * coat_dfg.x + coat_dfg.y) * s.coat_energy, 0.0, 1.0);
            float base_albedo = rikka_luma(lerp(s.rho * (1.0 - s.transmission), s.metal_albedo, s.metalness));
            float internal_rough = 1.0 - (1.0 - rikka_pbr_average_fresnel(s.coat_ior)) / (s.coat_ior * s.coat_ior);
            float internal_smooth = rikka_pbr_schlick(s.coat_f0, coat_n_dot_v);
            float dielectric_rough = lerp(1.0, rough, clamp(s.specular_weight * f0, 0.0, 1.0));
            float base_rough = lerp(dielectric_rough, rough, s.metalness);
            float k = lerp(internal_smooth, internal_rough, base_rough);
            float darkening = (1.0 - k) / max(1.0 - base_albedo * k, 1e-4);
            float modulated = lerp(1.0, darkening, s.coat * clamp(d.coat_darkening, 0.0, 1.0));
            float3 absorbed = rikka_openpbr_coat_transmittance(s.coat_color, s.coat_ior, coat_n_dot_v, coat_n_dot_v);
            s.coat_base = lerp(float3(1.0), absorbed * (1.0 - s.coat_albedo), s.coat) * modulated;
        }

        s.fuzz = rikka_feature(RIKKA_PBR_FUZZ) ? clamp(d.fuzz_weight, 0.0, 1.0) : 0.0;
        s.fuzz_color = clamp(d.fuzz_color, float3(0.0), float3(1.0));
        s.fuzz_roughness = clamp(d.fuzz_roughness, 0.0, 1.0);
        s.fuzz_albedo = s.fuzz > 0.0 ? rikka_pbr_sheen_albedo(s.n_dot_v, s.fuzz_roughness) : 0.0;

        s.cavity = clamp(d.cavity, 0.0, 1.0);
        s.occlusion = surface.occlusion;
        s.emission = surface.emission;
        s.opacity = surface.alpha;
        s.scattered = context.frame.look.scene.scatter != RIKKA_NONE && s.subsurface > 0.0 && !s.thin_walled
            && !d.unscattered;
        s.layer = (1.0 - s.fuzz * s.fuzz_albedo) * s.coat_base;
        float3 opaque = (1.0 - s.transmission) * (1.0 - s.metalness) * (float3(1.0) - s.dielectric_albedo);
        s.diffuse_weight = opaque * (1.0 - s.subsurface);
        s.scatter_weight = opaque * s.subsurface;
        s.lit = float3(0.0);
        s.scatter = float3(0.0);
        return s;
    }

    [constref]
    float3 base_weight(State s) {
        return s.layer;
    }

    [constref]
    RikkaOpenPbrDiffuse diffuse(State s, float3 diffuse, float3 subsurface) {
        float3 scattered = subsurface * s.scatter_weight;
        RikkaOpenPbrDiffuse d;
        d.lit = diffuse * s.diffuse_weight;
        d.scatter = float3(0.0);
        if (s.scattered) {
            d.scatter = scattered;
        } else {
            d.lit += scattered;
        }
        return d;
    }

    [constref]
    void direct(inout State s, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        if (light.area()) {
            area(s, light, shadow, context);
            return;
        }
        float3 l = s.basis.local(light.direction);
        if (light.attenuation <= 0.0 || l.z <= 0.0) {
            return;
        }
        float3 visible = shadow.transmittance();
        if (all(visible <= float3(0.0))) {
            return;
        }
        float3 incident = light.radiance() * light.attenuation * visible;
        float3 h = normalize(s.v + l);
        float v_dot_h = clamp(dot(s.v, h), 0.0, 1.0);

        float lobe = rikka_pbr_ggx(h, s.alpha) * rikka_pbr_visibility(s.v, l, s.alpha);
        if (s.second_lobe > 0.0) {
            float second = rikka_pbr_ggx(h, s.second_alpha) * rikka_pbr_visibility(s.v, l, s.second_alpha);
            lobe = lerp(lobe, second, s.second_lobe);
        }
        lobe *= l.z * s.cavity;
        float3 fresnel = float3(0.0);
        if (s.metalness < 1.0) {
            float3 f_dielectric = s.specular_tint * rikka_pbr_schlick(s.dielectric_f0, float3(s.dielectric_f90), v_dot_h);
            if (s.film_weight > 0.0) {
                f_dielectric = lerp(f_dielectric, s.specular_tint * s.film, s.film_weight);
            }
            fresnel += f_dielectric * s.dielectric_energy * (1.0 - s.metalness);
        }
        if (s.metalness > 0.0) {
            float3 f_metal = s.specular_weight * rikka_pbr_f82(s.metal_f0, s.metal_tint, v_dot_h);
            if (s.film_weight > 0.0) {
                f_metal = lerp(f_metal, s.specular_weight * s.film_metal, s.film_weight);
            }
            fresnel += f_metal * s.metal_energy * s.metalness;
        }
        float3 specular = fresnel * lobe;
        RikkaOpenPbrDiffuse under = diffuse(s, rikka_pbr_eon(s.rho, s.diffuse_roughness, l, s.v) * l.z,
            s.subsurface_color * (l.z / RIKKA_PI));
        float3 base = specular + under.lit;
        float3 scattered = under.scatter;

        float coat = 0.0;
        if (s.coat > 0.0) {
            RikkaPbrBasis coat_basis = rikka_pbr_basis(s.coat_normal, s.basis.t);
            float3 coat_l = coat_basis.local(light.direction);
            float3 coat_v = coat_basis.local(s.view);
            coat_v.z = max(coat_v.z, RIKKA_PBR_EPSILON);
            if (coat_l.z > 0.0) {
                float3 coat_h = normalize(coat_v + coat_l);
                coat = s.coat * rikka_pbr_schlick(s.coat_f0, clamp(dot(coat_v, coat_h), 0.0, 1.0)) * s.coat_energy
                    * rikka_pbr_ggx(coat_h, s.coat_alpha) * rikka_pbr_visibility(coat_v, coat_l, s.coat_alpha) * coat_l.z;
            }
            float3 through = rikka_openpbr_coat_transmittance(s.coat_color, s.coat_ior, l.z, s.n_dot_v);
            float3 viewed = rikka_openpbr_coat_transmittance(s.coat_color, s.coat_ior, s.n_dot_v, s.n_dot_v);
            float3 ratio = lerp(float3(1.0), through / max(viewed, float3(1e-4)), s.coat);
            base *= ratio;
            scattered *= ratio;
        }
        float through_fuzz = 1.0 - s.fuzz * s.fuzz_albedo;
        float3 fuzz = float3(0.0);
        if (s.fuzz > 0.0) {
            fuzz = s.fuzz * s.fuzz_color * s.fuzz_albedo
                * rikka_pbr_sheen_lobe(s.basis.n, s.view, light.direction, s.fuzz_roughness);
        }
        float3 weight = base_weight(s);
        s.lit += incident * (fuzz + through_fuzz * coat + weight * base);
        s.scatter += incident * weight * scattered;
        if (s.wet.film > 0.0) {
            s.lit += incident * s.wet.film * rikka_wet_film_light(s.film_normal, s.view, light.direction);
        }
    }

    [constref]
    void area(inout State s, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        RikkaFrameRef frame = context.frame;
        float3 n = s.basis.n;
        float irradiance = light.irradiance(frame, n);
        if (irradiance <= 0.0 && light.window <= 0.0) {
            return;
        }
        float3 visible = shadow.transmittance();
        if (all(visible <= float3(0.0))) {
            return;
        }
        float3 radiance = light.radiance() * visible;
        float lobe = light.specular(frame, rikka_ltc(frame, rikka_openpbr_ltc_roughness(s.alpha), s.n_dot_v), n, s.view);
        if (s.second_lobe > 0.0) {
            float second = light.specular(frame, rikka_ltc(frame, rikka_openpbr_ltc_roughness(s.second_alpha), s.n_dot_v), n, s.view);
            lobe = lerp(lobe, second, s.second_lobe);
        }
        float3 specular = lerp(s.dielectric_albedo, s.metal_albedo, s.metalness) * (lobe * s.cavity);
        float cosine = irradiance * (1.0 / RIKKA_PI);
        RikkaOpenPbrDiffuse under = diffuse(s, rikka_pbr_eon_albedo(s.rho, s.diffuse_roughness, s.n_dot_v) * cosine,
            s.subsurface_color * cosine);
        float3 base = specular + under.lit;
        float3 scattered = under.scatter;

        float coat = 0.0;
        if (s.coat > 0.0) {
            float coat_n_dot_v = max(dot(s.coat_normal, s.view), RIKKA_PBR_EPSILON);
            RikkaLtc coat_ltc = rikka_ltc(frame, rikka_openpbr_ltc_roughness(s.coat_alpha), coat_n_dot_v);
            coat = s.coat * s.coat_albedo * light.specular(frame, coat_ltc, s.coat_normal, s.view);
            float mu = clamp(dot(n, light.direction), RIKKA_PBR_EPSILON, 1.0);
            float3 through = rikka_openpbr_coat_transmittance(s.coat_color, s.coat_ior, mu, s.n_dot_v);
            float3 viewed = rikka_openpbr_coat_transmittance(s.coat_color, s.coat_ior, s.n_dot_v, s.n_dot_v);
            float3 ratio = lerp(float3(1.0), through / max(viewed, float3(1e-4)), s.coat);
            base *= ratio;
            scattered *= ratio;
        }
        float through_fuzz = 1.0 - s.fuzz * s.fuzz_albedo;
        float3 fuzz = float3(0.0);
        if (s.fuzz > 0.0) {
            fuzz = s.fuzz * s.fuzz_color * s.fuzz_albedo
                * light.specular(frame, rikka_pbr_sheen_ltc(s.n_dot_v, s.fuzz_roughness), n, s.view);
        }
        float3 weight = base_weight(s);
        s.lit += radiance * (fuzz + through_fuzz * coat + weight * base);
        s.scatter += radiance * weight * scattered;
        if (s.wet.film > 0.0) {
            s.lit += radiance * light.attenuation * s.wet.film * rikka_wet_film_light(s.film_normal, s.view, light.direction);
        }
    }

    [constref]
    override void transmit(inout State s, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        if (s.subsurface <= 0.0 || light.attenuation <= 0.0) {
            return;
        }
        float back = -dot(s.basis.n, light.direction);
        if (back <= 0.0) {
            return;
        }
        float3 incident = light.light.color * light.light.intensity * light.attenuation;
        float3 opaque = (1.0 - s.transmission) * (1.0 - s.metalness) * s.subsurface * base_weight(s);
        if (s.thin_walled) {
            float forward = 0.5 * (1.0 + s.subsurface_anisotropy);
            s.lit += incident * opaque * s.subsurface_color * (forward * back / RIKKA_PI);
            return;
        }
        if (s.thickness <= 0.0) {
            return;
        }
        RikkaShadowSample measured = shadow.sample();
        float path = measured.blocked && measured.blocker > 0.0 ? measured.blocker : s.thickness;
        float3 reach = max(s.subsurface_radius, float3(1e-5));
        float3 through = exp(-path / reach) * measured.tint;
        float3 transmitted = incident * opaque * s.subsurface_color * through * (back / RIKKA_PI);
        if (s.scattered) {
            s.scatter += transmitted;
        } else {
            s.lit += transmitted;
        }
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
        float3 n = s.basis.n;
        float3 reflected = reflect(-s.view, n);
        float3 direction = s.anisotropy > 0.0
            ? rikka_pbr_bent_reflection(s.basis, s.view, s.roughness, s.anisotropy)
            : rikka_pbr_dominant(n, s.view, s.roughness);
        float specular_occlusion = indirect.specular_occlusion(n, s.view, s.roughness, s.occlusion)
            * rikka_pbr_horizon(reflected, s.geometric_normal) * s.cavity;
        float3 radiance = indirect.specular(direction, s.roughness);
        if (s.second_lobe > 0.0) {
            float second = sqrt(sqrt(s.second_alpha.x * s.second_alpha.y));
            radiance = lerp(radiance, indirect.specular(rikka_pbr_dominant(n, s.view, second), second), s.second_lobe);
        }
        float3 specular = lerp(s.dielectric_albedo, s.metal_albedo, s.metalness) * radiance * specular_occlusion;
        float3 irradiance = indirect.diffuse(n, s.rho, s.occlusion);

        RikkaOpenPbrDiffuse under = diffuse(s, irradiance * rikka_pbr_eon_albedo(s.rho, s.diffuse_roughness, s.n_dot_v),
            irradiance * s.subsurface_color);
        float3 base = (specular + under.lit) * base_weight(s);
        float3 scattered = under.scatter * base_weight(s);

        float3 coat = float3(0.0);
        if (s.coat > 0.0) {
            float3 coat_reflected = reflect(-s.view, s.coat_normal);
            float coat_occlusion = indirect.specular_occlusion(s.coat_normal, s.view, s.coat_roughness, s.occlusion)
                * rikka_pbr_horizon(coat_reflected, s.geometric_normal);
            coat = s.coat * s.coat_albedo * coat_occlusion
                * indirect.specular(rikka_pbr_dominant(s.coat_normal, s.view, s.coat_roughness), s.coat_roughness);
        }
        float3 fuzz = s.fuzz * s.fuzz_color * s.fuzz_albedo * irradiance;
        float fuzz_through = 1.0 - s.fuzz * s.fuzz_albedo;
        float3 emitted = s.emission * fuzz_through;
        if (s.coat > 0.0) {
            emitted *= lerp(float3(1.0), rikka_openpbr_coat_transmittance(s.coat_color, s.coat_ior, s.n_dot_v, s.n_dot_v), s.coat);
        }

        s.lit += base + fuzz + fuzz_through * coat + emitted;
        s.scatter += scattered;
        if (s.wet.film > 0.0) {
            float3 film = indirect.specular(reflect(-s.view, s.film_normal), RIKKA_WET_FILM_ROUGHNESS) * s.film_view;
            s.lit += film * indirect.specular_occlusion(s.film_normal, s.view, RIKKA_WET_FILM_ROUGHNESS, s.occlusion);
        }
    }

    [constref]
    float2 refracted_uv(State s, RikkaPixel pixel, RikkaContext context, float ior) {
        RikkaViewRef view = context.view;
        float2 here = pixel.frag_coord.xy * view.inverse_viewport;
        if (s.thin_walled || s.thickness <= 0.0) {
            return here;
        }
        float3 bent = refract(-s.view, s.basis.n, 1.0 / max(ior, 1.0));
        float4 clip = mul(view.view_projection, float4(pixel.world_position + bent * s.thickness, 1.0));
        if (clip.w <= 1e-4) {
            return here;
        }
        float2 moved = clip.xy / clip.w * 0.5 + 0.5;
        if (any(moved < float2(0.0)) || any(moved > float2(1.0))) {
            return here;
        }
        uint scene_depth = context.frame.look.scene.depth;
        if (scene_depth != RIKKA_NONE) {
            float behind = rikka_read_depth(scene_depth,
                min(int2(moved * view.viewport), int2(view.viewport) - 1));
            if (behind > 0.0 && rikka_linear_depth(view, behind) < pixel.view_depth) {
                return here;
            }
        }
        return moved;
    }

    [constref]
    float3 refracted_at(RikkaPixel pixel, RikkaContext context, uint image, float2 uv, float spread) {
        RikkaViewRef view = context.view;
        float spin = rikka_temporal_noise(view, pixel.frag_coord.xy) * 6.2831853;
        float2 aspect = float2(view.viewport.y * view.inverse_viewport.x, 1.0);
        float3 sum = float3(0.0);
        for (float tap = 0.0; tap < RIKKA_OPENPBR_REFRACTION_TAPS; tap += 1.0) {
            float radius = sqrt((tap + 0.5) / RIKKA_OPENPBR_REFRACTION_TAPS) * spread;
            float angle = spin + tap * 2.3999632;
            float2 at = clamp(uv + float2(cos(angle), sin(angle)) * radius * aspect, float2(0.0), float2(1.0));
            sum += rikka_textures[NonUniformResourceIndex(image)]
                .SampleLevel(rikka_render_uv(view, at, 1u), 0.0).rgb;
        }
        return sum / RIKKA_OPENPBR_REFRACTION_TAPS;
    }

    [constref]
    float3 refracted(State s, RikkaPixel pixel, RikkaContext context, uint image) {
        float spread = rikka_pbr_sq(s.roughness) * RIKKA_OPENPBR_REFRACTION_BLUR * context.view.inverse_viewport.y;
        float3 color;
        if (s.dispersion > 0.0 && !s.thin_walled) {
            float b = (s.eta - 1.0) * s.dispersion / (1.0 / (RIKKA_OPENPBR_FRAUNHOFER_F * RIKKA_OPENPBR_FRAUNHOFER_F)
                - 1.0 / (RIKKA_OPENPBR_FRAUNHOFER_C * RIKKA_OPENPBR_FRAUNHOFER_C));
            float a = s.eta - b / (RIKKA_OPENPBR_FRAUNHOFER_D * RIKKA_OPENPBR_FRAUNHOFER_D);
            float3 iors = a + b / (RIKKA_OPENPBR_WAVELENGTHS * RIKKA_OPENPBR_WAVELENGTHS);
            color.r = refracted_at(pixel, context, image, refracted_uv(s, pixel, context, iors.r), spread).r;
            color.g = refracted_at(pixel, context, image, refracted_uv(s, pixel, context, iors.g), spread).g;
            color.b = refracted_at(pixel, context, image, refracted_uv(s, pixel, context, iors.b), spread).b;
        } else {
            color = refracted_at(pixel, context, image, refracted_uv(s, pixel, context, s.eta), spread);
        }
        float3 absorbed = s.transmission_depth > 0.0 && !s.thin_walled
            ? pow(max(s.transmission_color, float3(1e-4)), float3(s.thickness / s.transmission_depth))
            : s.transmission_color;
        return color * absorbed;
    }

    [constref]
    float4 resolve(State s, RikkaPixel pixel, RikkaContext context) {
        float alpha = rikka_coverage(context, s.opacity);
        float3 lit = s.lit;
        if (s.transmission > 0.0 && (context.instance.flags & RIKKA_TRANSPARENT) != 0u) {
            float3 through = s.transmission * (1.0 - s.metalness) * (float3(1.0) - s.dielectric_albedo) * base_weight(s);
            uint image = context.frame.look.scene.refraction;
            if (image != RIKKA_NONE) {
                lit += through * refracted(s, pixel, context, image);
            } else {
                alpha *= 1.0 - rikka_luma(through * s.transmission_color);
            }
        }
        return float4(lit, alpha);
    }

    [constref]
    override RikkaGBuffer gbuffer(RikkaContext context) {
        return rikka_gbuffer(context, gloss);
    }

    [constref]
    override RikkaScatter scattered(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        if (surface.model.subsurface_weight <= 0.0 || surface.model.thin_walled || surface.model.unscattered) {
            return rikka_no_scatter();
        }
        State s = rikka_gather(this, pixel, surface, context);
        RikkaScatter scatter;
        scatter.diffuse = float4(s.scatter, 1.0);
        scatter.profile = float4(s.subsurface_radius / rikka_openpbr_burley_shape(s.subsurface_color), 1.0);
        return scatter;
    }
};
