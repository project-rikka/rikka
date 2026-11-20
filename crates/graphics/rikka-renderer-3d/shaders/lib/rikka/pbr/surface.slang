__exported import rikka.pbr.openpbr;

float3 rikka_pbr_tangent_normal(RikkaTexture texture, float2 uv, float scale) {
    float3 tangent_normal = rikka_unpack_normal(texture.sampled(uv));
    tangent_normal.xy *= scale;
    return normalize(tangent_normal);
}

float3 rikka_pbr_anisotropy_tangent(RikkaPixel pixel, float3 normal, float2 direction, float rotation) {
    float3 t = normalize(pixel.tangent.xyz - normal * dot(normal, pixel.tangent.xyz));
    float3 b = cross(normal, t) * pixel.tangent.w;
    float c = cos(rotation);
    float s = sin(rotation);
    float2 turned = float2(direction.x * c - direction.y * s, direction.x * s + direction.y * c);
    return turned.x * t + turned.y * b;
}

struct RikkaOpenPbrSurface : ISurface<RikkaOpenPbrData> {
    [Default(1.0)] [Range(0.0, 1.0)] [Group("base")] [Order(0)] [Label("Weight")] [Role("albedo_weight")] float base_weight;
    [Color] [Srgb] [Default(0.8, 0.8, 0.8)] [Group("base")] [Label("Color")] [Role("albedo")]
    [Preset("velvet", 0.18, 0.03, 0.06)] [Preset("silk", 0.55, 0.08, 0.12)] [Preset("satin", 0.08, 0.1, 0.3)]
    [Preset("brushed_metal", 0.91, 0.92, 0.92)] [Preset("car_paint", 0.6, 0.02, 0.02)] [Preset("glass", 1.0, 1.0, 1.0)]
    float3 base_color;
    [Texture("colour")] [Group("base")] [Role("albedo_texture")] RikkaTexture base_color_texture;
    [Default(0.0)] [Range(0.0, 1.0)] [Group("base")] [Label("Metalness")] [Role("metalness")] [Preset("brushed_metal", 1.0)] [Preset("car_paint", 0.3)]
    float base_metalness;
    [Default(0.0)] [Range(0.0, 1.0)] [Group("base")] [Label("Diffuse roughness")] [Preset("velvet|silk|satin", 0.5)]
    float base_diffuse_roughness;
    [Texture("data")] [Group("base")] [Role("metalness_texture")] RikkaTexture metal_roughness_texture;

    [Default(1.0)] [SoftRange(0.0, 1.0)] [Group("specular")] [Label("Weight")] [Preset("velvet", 0.4)] float specular_weight;
    [Texture("data")] [Group("specular")] RikkaTexture specular_weight_texture;
    [Color] [Srgb] [Default(1.0, 1.0, 1.0)] [Group("specular")] [Label("Color")] [Preset("silk", 1.0, 0.8, 0.82)]
    [Preset("brushed_metal", 0.98, 0.98, 1.0)]
    float3 specular_color;
    [Texture("colour")] [Group("specular")] RikkaTexture specular_color_texture;
    [Default(0.3)] [Range(0.0, 1.0)] [Group("specular")] [Label("Roughness")] [Preset("velvet", 0.8)] [Preset("silk", 0.35)]
    [Preset("satin", 0.3)] [Preset("brushed_metal", 0.35)] [Preset("car_paint", 0.4)] [Preset("glass", 0.0)]
    float specular_roughness;
    [Default(0.0)] [Range(0.0, 1.0)] [Group("specular")] [Label("Anisotropy")] [Preset("silk", 0.8)] [Preset("satin", 0.6)]
    [Preset("brushed_metal", 0.75)] [Feature(5)]
    float specular_roughness_anisotropy;
    [Default(0.0)] [Group("specular")] [Hint("angle")] [Label("Anisotropy rotation")] float specular_rotation;
    [Texture("data")] [Group("specular")] RikkaTexture anisotropy_texture;
    [Default(1.5)] [SoftRange(1.0, 3.0)] [Group("specular")] [Label("IOR")] float specular_ior;

    [Default(0.0)] [Range(0.0, 1.0)] [Group("transmission")] [Label("Weight")] [Role("transmission")] [Feature(4)] [Preset("glass", 1.0)] float transmission_weight;
    [Texture("data")] [Group("transmission")] RikkaTexture transmission_texture;
    [Color] [Srgb] [Default(1.0, 1.0, 1.0)] [Group("transmission")] [Label("Color")] float3 transmission_color;
    [Default(0.0)] [SoftRange(0.0, 1.0)] [Group("transmission")] [Label("Depth")] float transmission_depth;
    [Default(0.0)] [Range(0.0, 1.0)] [Group("transmission")] [Label("Dispersion scale")] float transmission_dispersion_scale;
    [Default(20.0)] [SoftRange(9.0, 91.0)] [Group("transmission")] [Label("Abbe number")] [Hint("advanced")]
    float transmission_dispersion_abbe_number;

    [Default(0.0)] [Range(0.0, 1.0)] [Group("subsurface")] [Label("Weight")] [Role("subsurface")] [Feature(3)] float subsurface_weight;
    [Color] [Srgb] [Default(0.8, 0.8, 0.8)] [Group("subsurface")] [Label("Color")] float3 subsurface_color;
    [Default(1.0)] [SoftRange(0.0, 1.0)] [Group("subsurface")] [Label("Radius")] [Hint("logarithmic")] float subsurface_radius;
    [Color] [Default(1.0, 0.5, 0.25)] [Group("subsurface")] [Label("Radius scale")] float3 subsurface_radius_scale;
    [Default(0.0)] [Range(-1.0, 1.0)] [Group("subsurface")] [Label("Anisotropy")] float subsurface_scatter_anisotropy;

    [Default(0.0)] [Range(0.0, 1.0)] [Group("coat")] [Label("Weight")] [Feature(0)] [Preset("car_paint", 1.0)] float coat_weight;
    [Texture("data")] [Group("coat")] RikkaTexture coat_texture;
    [Color] [Srgb] [Default(1.0, 1.0, 1.0)] [Group("coat")] [Label("Color")] float3 coat_color;
    [Default(0.0)] [Range(0.0, 1.0)] [Group("coat")] [Label("Roughness")] [Preset("car_paint", 0.03)] float coat_roughness;
    [Texture("data")] [Group("coat")] RikkaTexture coat_roughness_texture;
    [Default(0.0)] [Range(0.0, 1.0)] [Group("coat")] [Label("Anisotropy")] float coat_roughness_anisotropy;
    [Default(1.6)] [SoftRange(1.0, 3.0)] [Group("coat")] [Label("IOR")] float coat_ior;
    [Default(1.0)] [Range(0.0, 1.0)] [Group("coat")] [Label("Darkening")] float coat_darkening;
    [Texture("normal")] [Group("coat")] RikkaTexture coat_normal_texture;
    [Default(1.0)] [SoftRange(0.0, 2.0)] [Group("coat")] float coat_normal_scale;

    [Default(0.0)] [Range(0.0, 1.0)] [Group("fuzz")] [Label("Weight")] [Feature(1)] [Preset("velvet", 1.0)] [Preset("silk", 0.25)] [Preset("satin", 0.4)]
    float fuzz_weight;
    [Color] [Srgb] [Default(1.0, 1.0, 1.0)] [Group("fuzz")] [Label("Color")] [Preset("velvet", 0.75, 0.2, 0.3)] [Preset("silk", 1.0, 0.85, 0.85)]
    [Preset("satin", 0.5, 0.6, 1.0)]
    float3 fuzz_color;
    [Texture("colour")] [Group("fuzz")] RikkaTexture fuzz_color_texture;
    [Default(0.5)] [Range(0.0, 1.0)] [Group("fuzz")] [Label("Roughness")] [Preset("velvet", 0.35)] [Preset("silk", 0.6)] [Preset("satin", 0.45)]
    float fuzz_roughness;
    [Texture("data")] [Group("fuzz")] RikkaTexture fuzz_roughness_texture;

    [Default(0.0)] [Range(0.0, 1.0)] [Group("thin_film")] [Label("Weight")] [Feature(2)] float thin_film_weight;
    [Texture("data")] [Group("thin_film")] RikkaTexture thin_film_texture;
    [Default(0.5)] [SoftRange(0.0, 1.0)] [Group("thin_film")] [Label("Thickness")] float thin_film_thickness;
    [Default(0.1)] [SoftRange(0.0, 1.0)] [Group("thin_film")] [Hint("advanced")] float thin_film_thickness_minimum;
    [Texture("data")] [Group("thin_film")] RikkaTexture thin_film_thickness_texture;
    [Default(1.4)] [SoftRange(1.0, 3.0)] [Group("thin_film")] [Label("IOR")] float thin_film_ior;

    [Default(0.0)] [SoftRange(0.0, 1000.0)] [Group("emission")] [Label("Luminance")] [Role("emission_luminance")] float emission_luminance;
    [Color] [Srgb] [Default(1.0, 1.0, 1.0)] [Group("emission")] [Label("Color")] [Role("emission")] float3 emission_color;
    [Texture("colour")] [Group("emission")] [Role("emission_texture")] RikkaTexture emission_texture;

    [Default(1.0)] [Range(0.0, 1.0)] [Group("geometry")] [Label("Opacity")] float geometry_opacity;
    [Default(0.0)] [Group("geometry")] [Label("Thin walled")] bool geometry_thin_walled;
    [Default(0.0)] [SoftRange(0.0, 0.5)] [Group("geometry")] [Label("Thickness")] [Preset("glass", 0.05)] float geometry_thickness;
    [Texture("data")] [Group("geometry")] RikkaTexture thickness_texture;
    [Texture("normal")] [Group("geometry")] RikkaTexture normal_texture;
    [Default(1.0)] [SoftRange(0.0, 2.0)] [Group("geometry")] float normal_scale;
    [Texture("data")] [Group("geometry")] RikkaTexture occlusion_texture;
    [Default(1.0)] [Range(0.0, 1.0)] [Group("geometry")] float occlusion_strength;
    RikkaUvTransform uv;

    [constref]
    RikkaSurfaceData<RikkaOpenPbrData> evaluate(RikkaPixel pixel, RikkaContext context) {
        float2 at = uv.apply(pixel.uv);
        float4 base = float4(base_color, geometry_opacity) * pixel.color * base_color_texture.sampled(at);
        float4 metal_rough = metal_roughness_texture.sampled(at);
        RikkaOpenPbrData d = rikka_openpbr_data();
        d.metalness = base_metalness * metal_rough.b;
        d.diffuse_roughness = base_diffuse_roughness;
        d.specular_weight = specular_weight * specular_weight_texture.sampled(at).a;
        d.specular_color = specular_color * specular_color_texture.sampled(at).rgb;
        d.specular_roughness = specular_roughness * metal_rough.g;
        d.specular_anisotropy = specular_roughness_anisotropy;
        d.specular_ior = specular_ior;
        RikkaSurfaceData<RikkaOpenPbrData> s = rikka_surface_data(pixel, d);
        if (normal_texture.bound()) {
            s.normal = pixel.normal_map(rikka_pbr_tangent_normal(normal_texture, at, normal_scale));
        }
        if (specular_roughness_anisotropy > 0.0) {
            float2 direction = float2(1.0, 0.0);
            if (anisotropy_texture.bound()) {
                float3 texel = anisotropy_texture.sampled(at).rgb;
                direction = texel.rg * 2.0 - 1.0;
                d.specular_anisotropy *= texel.b;
            }
            d.tangent = rikka_pbr_anisotropy_tangent(pixel, normalize(s.normal), direction, specular_rotation);
        }
        d.transmission_weight = transmission_weight * transmission_texture.sampled(at).r;
        d.transmission_color = transmission_color;
        d.transmission_depth = transmission_depth;
        d.dispersion = transmission_dispersion_scale / max(transmission_dispersion_abbe_number, 1.0);
        d.thickness = geometry_thickness * thickness_texture.sampled(at).g;
        d.subsurface_weight = subsurface_weight;
        d.subsurface_color = subsurface_color;
        d.subsurface_radius = subsurface_radius * subsurface_radius_scale;
        d.subsurface_anisotropy = subsurface_scatter_anisotropy;
        d.coat_weight = coat_weight * coat_texture.sampled(at).r;
        d.coat_color = coat_color;
        d.coat_roughness = coat_roughness * coat_roughness_texture.sampled(at).g;
        d.coat_anisotropy = coat_roughness_anisotropy;
        d.coat_ior = coat_ior;
        d.coat_darkening = coat_darkening;
        if (coat_normal_texture.bound()) {
            d.coat_normal = pixel.normal_map(rikka_pbr_tangent_normal(coat_normal_texture, at, coat_normal_scale));
        }
        d.fuzz_weight = fuzz_weight;
        d.fuzz_color = fuzz_color * fuzz_color_texture.sampled(at).rgb;
        d.fuzz_roughness = fuzz_roughness * fuzz_roughness_texture.sampled(at).a;
        d.thin_film_weight = thin_film_weight * thin_film_texture.sampled(at).r;
        d.thin_film_thickness = thin_film_thickness_texture.bound()
            ? lerp(thin_film_thickness_minimum, thin_film_thickness, thin_film_thickness_texture.sampled(at).g)
            : thin_film_thickness;
        d.thin_film_ior = thin_film_ior;
        d.thin_walled = geometry_thin_walled;
        s.model = d;
        s.albedo = base.rgb * base_weight;
        s.alpha = base.a;
        s.occlusion = lerp(1.0, occlusion_texture.sampled(at).r, occlusion_strength);
        s.emission = emission_color * (emission_luminance / RIKKA_NITS_PER_UNIT) * emission_texture.sampled(at).rgb;
        return s;
    }

    [constref]
    override float coverage(RikkaCoverage coverage, RikkaContext context) {
        return geometry_opacity * (base_color_texture.bound() ? base_color_texture.sampled(uv.apply(coverage.uv)).a : 1.0);
    }

    [constref]
    override float3 transmittance(RikkaCoverage at, RikkaContext context) {
        float2 where = uv.apply(at.uv);
        float opacity = coverage(at, context);
        float metal = clamp(base_metalness * metal_roughness_texture.sampled(where).b, 0.0, 1.0);
        float weight = clamp(transmission_weight * transmission_texture.sampled(where).r, 0.0, 1.0) * (1.0 - metal);
        float3 color = clamp(transmission_color, float3(0.0), float3(1.0));
        float thickness = geometry_thickness * thickness_texture.sampled(where).g;
        if (!geometry_thin_walled && transmission_depth > 0.0) {
            color = pow(max(color, float3(1e-4)), float3(thickness / transmission_depth));
        }
        float reflected = rikka_pbr_f0(rikka_pbr_modulated_eta(max(specular_ior, 1e-3), max(specular_weight, 0.0)));
        float3 passed = weight * color * rikka_pbr_sq(1.0 - reflected);
        return float3(1.0 - opacity) + opacity * passed;
    }
};
