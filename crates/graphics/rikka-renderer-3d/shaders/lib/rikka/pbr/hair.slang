__exported import rikka.pbr.brdf;
__exported import rikka.standard;

static const float RIKKA_HAIR_IOR = 1.55;
static const float RIKKA_HAIR_AREA = 0.0;

struct RikkaHairData {
    float3 tangent;
    float roughness;
    float shift;
    float specular;
    float backlit;
    float scatter;
};

struct RikkaHairState {
    float3 tangent;
    float3 normal;
    float3 geometric_normal;
    float3 view;
    float3 color;
    float roughness;
    float shift;
    float specular;
    float backlit;
    float scatter;
    float occlusion;
    float3 emission;
    float alpha;
    float3 lit;
};

float rikka_hair_gaussian(float width, float theta) {
    return exp(-0.5 * theta * theta / (width * width)) / (sqrt(2.0 * RIKKA_PI) * width);
}

float rikka_hair_fresnel(float cosine) {
    float f0 = rikka_pbr_sq((1.0 - RIKKA_HAIR_IOR) / (1.0 + RIKKA_HAIR_IOR));
    return f0 + (1.0 - f0) * rikka_pbr_pow5(1.0 - cosine);
}

float3 rikka_hair_marschner(RikkaHairState s, float3 l) {
    float3 v = s.view;
    float3 t = s.tangent;
    float v_dot_l = dot(v, l);
    float sin_l = clamp(dot(t, l), -1.0, 1.0);
    float sin_v = clamp(dot(t, v), -1.0, 1.0);
    float cos_d = cos(0.5 * abs(asin(sin_v) - asin(sin_l)));
    float3 lp = l - sin_l * t;
    float3 vp = v - sin_v * t;
    float cos_phi = dot(lp, vp) * rsqrt(dot(lp, lp) * dot(vp, vp) + 1e-4);
    float cos_half_phi = sqrt(clamp(0.5 + 0.5 * cos_phi, 0.0, 1.0));
    float n_prime = 1.19 / cos_d + 0.36 * cos_d;
    float r2 = rikka_pbr_sq(clamp(s.roughness, 0.02, 1.0));
    float3 alpha = float3(-2.0 * s.shift, s.shift, 4.0 * s.shift);
    float3 width = float3(RIKKA_HAIR_AREA + r2, RIKKA_HAIR_AREA + 0.5 * r2, RIKKA_HAIR_AREA + 2.0 * r2);

    float3 result = float3(0.0);
    {
        float sa = sin(alpha.x);
        float ca = cos(alpha.x);
        float shift_v = 2.0 * sa * (ca * cos_half_phi * sqrt(1.0 - sin_v * sin_v) + sa * sin_v);
        float m = rikka_hair_gaussian(width.x * sqrt(2.0) * cos_half_phi, sin_l + sin_v - shift_v);
        float n = 0.25 * cos_half_phi;
        float f = rikka_hair_fresnel(sqrt(clamp(0.5 + 0.5 * v_dot_l, 0.0, 1.0)));
        result += s.specular * m * n * f * lerp(1.0, s.backlit, clamp(-v_dot_l, 0.0, 1.0));
    }
    {
        float m = rikka_hair_gaussian(width.y, sin_l + sin_v - alpha.y);
        float a = 1.0 / n_prime;
        float h = cos_half_phi * (1.0 + a * (0.6 - 0.8 * cos_phi));
        float f = rikka_hair_fresnel(cos_d * sqrt(clamp(1.0 - h * h, 0.0, 1.0)));
        float3 transmitted = pow(max(s.color, float3(1e-4)), float3(0.5 * sqrt(clamp(1.0 - rikka_pbr_sq(h * a), 0.0, 1.0)) / cos_d));
        float n = exp(-3.65 * cos_phi - 3.98);
        result += m * n * rikka_pbr_sq(1.0 - f) * transmitted * s.backlit;
    }
    {
        float m = rikka_hair_gaussian(width.z, sin_l + sin_v - alpha.z);
        float f = rikka_hair_fresnel(cos_d * 0.5);
        float3 transmitted = pow(max(s.color, float3(1e-4)), float3(0.8 / cos_d));
        float n = exp(17.0 * cos_phi - 16.78);
        result += m * n * rikka_pbr_sq(1.0 - f) * f * transmitted;
    }
    return result;
}

float3 rikka_hair_diffuse(RikkaHairState s, float3 l) {
    float wrapped = clamp((dot(s.normal, l) + 1.0) / 4.0, 0.0, 1.0);
    float kajiya = sqrt(clamp(1.0 - rikka_pbr_sq(dot(s.tangent, l)), 0.0, 1.0));
    float3 tint = pow(max(s.color, float3(1e-4)), float3(1.0 - 0.5 * s.scatter));
    return tint * lerp(kajiya * 0.5, wrapped, s.scatter) / RIKKA_PI;
}

struct RikkaHairModel : IShadingModel {
    RikkaGloss gloss;

    typealias Data = RikkaHairData;
    typealias State = RikkaHairState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.normal = normalize(surface.normal);
        s.geometric_normal = pixel.geometric_normal;
        s.view = pixel.view;
        s.tangent = normalize(surface.model.tangent);
        s.color = clamp(surface.albedo, float3(0.0), float3(1.0));
        float kernel = rikka_pbr_filter_kernel(s.normal);
        s.roughness = sqrt(rikka_pbr_filtered_alpha(float2(rikka_pbr_sq(surface.model.roughness)), kernel).x);
        s.shift = surface.model.shift;
        s.specular = surface.model.specular;
        s.backlit = surface.model.backlit;
        s.scatter = clamp(surface.model.scatter, 0.0, 1.0);
        s.occlusion = surface.occlusion;
        s.emission = surface.emission;
        s.alpha = surface.alpha;
        s.lit = float3(0.0);
        return s;
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
        s.lit += incident * (rikka_hair_marschner(s, l) + rikka_hair_diffuse(s, l));
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
        float3 tint = pow(max(s.color, float3(1e-4)), float3(1.0 - 0.5 * s.scatter));
        float3 irradiance = indirect.diffuse(s.normal, tint, s.occlusion);
        float3 toward = normalize(s.view - s.tangent * dot(s.tangent, s.view) + s.normal * 0.5);
        float3 sheen = indirect.specular(reflect(-s.view, s.normal), s.roughness)
            * indirect.specular_occlusion(s.normal, s.view, s.roughness, s.occlusion);
        float3 marschner = rikka_hair_marschner(s, toward) * RIKKA_PI * 0.5;
        s.lit += irradiance * tint + sheen * marschner * 0.25 + s.emission;
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

struct RikkaHairSurface : ISurface<RikkaHairData> {
    [Color] [Srgb] [Default(0.32, 0.2, 0.12)] [Group("base")] [Label("Color")] [Role("albedo")]
    [Preset("blonde", 0.85, 0.62, 0.38)] [Preset("brown", 0.32, 0.2, 0.12)] [Preset("black", 0.04, 0.035, 0.03)] [Preset("red", 0.6, 0.18, 0.08)]
    float3 base_color;
    [Texture("colour")] [Group("base")] [Role("albedo_texture")] RikkaTexture base_color_texture;
    [Color] [Srgb] [Default(1.0, 1.0, 1.0)] [Group("base")] [Label("Tip tint")] float3 tip_color;
    [Default(0.35)] [Range(0.0, 1.0)] [Group("strand")] [Label("Roughness")] float roughness;
    [Default(0.035)] [SoftRange(-0.2, 0.2)] [Group("strand")] [Label("Cuticle tilt")] float shift;
    [Texture("data")] [Group("strand")] RikkaTexture shift_texture;
    [Default(0.5)] [SoftRange(0.0, 2.0)] [Group("strand")] float shift_variation;
    [Options("u|v")] [Default(1)] [Group("strand")] [Label("Strand direction")] uint direction;
    [Texture("data")] [Group("strand")] RikkaTexture flow_texture;
    [Default(1.0)] [SoftRange(0.0, 2.0)] [Group("strand")] [Label("Specular")] float specular;
    [Default(1.0)] [SoftRange(0.0, 2.0)] [Group("strand")] [Label("Backlit")] float backlit;
    [Default(0.4)] [Range(0.0, 1.0)] [Group("strand")] [Label("Scatter")] float scatter;
    [Texture("data")] [Group("geometry")] RikkaTexture occlusion_texture;
    [Default(1.0)] [Range(0.0, 1.0)] [Group("geometry")] float occlusion_strength;
    [Default(1.0)] [Range(0.0, 1.0)] [Group("geometry")] [Label("Opacity")] float geometry_opacity;
    RikkaUvTransform uv;

    [constref]
    RikkaSurfaceData<RikkaHairData> evaluate(RikkaPixel pixel, RikkaContext context) {
        float2 at = uv.apply(pixel.uv);
        float4 base = float4(base_color, geometry_opacity) * base_color_texture.sampled(at);
        float3 n = pixel.geometric_normal;
        float3 t = normalize(pixel.tangent.xyz - n * dot(n, pixel.tangent.xyz));
        float3 b = cross(n, t) * pixel.tangent.w;
        float2 flow = direction == 0u ? float2(1.0, 0.0) : float2(0.0, 1.0);
        if (flow_texture.bound()) {
            flow = flow_texture.sampled(at).rg * 2.0 - 1.0;
        }
        RikkaHairData d;
        d.tangent = normalize(flow.x * t + flow.y * b + n * 1e-4);
        d.roughness = roughness;
        d.shift = shift + (shift_texture.sampled(at).r - 0.5) * shift_variation * abs(shift) * 2.0;
        d.specular = specular;
        d.backlit = backlit;
        d.scatter = scatter;
        RikkaSurfaceData<RikkaHairData> s = rikka_surface_data(pixel, d);
        float along = direction == 0u ? at.x : at.y;
        s.albedo = base.rgb * lerp(float3(1.0), tip_color, clamp(along, 0.0, 1.0));
        s.alpha = base.a * pixel.color.a;
        s.occlusion = lerp(1.0, occlusion_texture.sampled(at).r, occlusion_strength);
        return s;
    }

    [constref]
    override float coverage(RikkaCoverage coverage, RikkaContext context) {
        return geometry_opacity * (base_color_texture.bound() ? base_color_texture.sampled(uv.apply(coverage.uv)).a : 1.0);
    }
};
