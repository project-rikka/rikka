__exported import rikka.pbr.surface;

float3 rikka_pbr_detail_normal(float3 base, float3 detail, float strength) {
    float3 scaled = normalize(float3(detail.xy * strength, detail.z));
    return normalize(float3(base.xy + scaled.xy, base.z * scaled.z));
}

struct RikkaSkinSurface : ISurface<RikkaOpenPbrData> {
    [Color] [Srgb] [Default(0.8, 0.6, 0.5)] [Group("base")] [Label("Color")] [Role("albedo")] float3 base_color;
    [Texture("colour")] [Group("base")] [Role("albedo_texture")] RikkaTexture base_color_texture;
    [Default(0.0)] [Range(0.0, 1.0)] [Group("base")] [Label("Diffuse roughness")] float base_diffuse_roughness;

    [Default(1.0)] [Range(0.0, 1.0)] [Group("subsurface")] [Label("Weight")] [Role("subsurface")] [Feature(3)] float subsurface_weight;
    [Color] [Srgb] [Default(1.0, 1.0, 1.0)] [Group("subsurface")] [Label("Tint")] float3 subsurface_tint;
    [Default(0.004)] [SoftRange(0.0, 0.02)] [Group("subsurface")] [Label("Radius")] [Hint("logarithmic")] float subsurface_radius;
    [Color] [Default(1.0, 0.37, 0.19)] [Group("subsurface")] [Label("Radius scale")] float3 subsurface_radius_scale;
    [Default(0.0)] [Range(-1.0, 1.0)] [Group("subsurface")] [Label("Anisotropy")] float subsurface_scatter_anisotropy;
    [Default(0.0)] [SoftRange(0.0, 0.05)] [Group("subsurface")] [Label("Thickness")] float thickness;
    [Texture("data")] [Group("subsurface")] RikkaTexture thickness_texture;

    [Default(1.0)] [SoftRange(0.0, 1.0)] [Group("specular")] [Label("Weight")] float specular_weight;
    [Default(0.5)] [Range(0.0, 1.0)] [Group("specular")] [Label("Roughness")] float specular_roughness;
    [Texture("data")] [Group("specular")] RikkaTexture roughness_texture;
    [Default(0.3)] [Range(0.0, 1.0)] [Group("specular")] [Label("Second roughness")] float specular_second_roughness;
    [Default(0.15)] [Range(0.0, 1.0)] [Group("specular")] [Label("Second lobe")] [Feature(6)] float specular_lobe_mix;
    [Default(1.4)] [SoftRange(1.0, 2.0)] [Group("specular")] [Label("IOR")] float specular_ior;
    [Texture("data")] [Group("specular")] RikkaTexture cavity_texture;
    [Default(1.0)] [Range(0.0, 1.0)] [Group("specular")] float cavity_strength;

    [Default(0.0)] [Range(0.0, 1.0)] [Group("fuzz")] [Label("Weight")] [Feature(1)] float fuzz_weight;
    [Color] [Srgb] [Default(1.0, 1.0, 1.0)] [Group("fuzz")] [Label("Color")] float3 fuzz_color;
    [Default(0.6)] [Range(0.0, 1.0)] [Group("fuzz")] [Label("Roughness")] float fuzz_roughness;

    [Texture("normal")] [Group("geometry")] RikkaTexture normal_texture;
    [Default(1.0)] [SoftRange(0.0, 2.0)] [Group("geometry")] float normal_scale;
    [Texture("normal")] [Group("geometry")] RikkaTexture detail_normal_texture;
    [Default(8.0)] [SoftRange(1.0, 64.0)] [Group("geometry")] float detail_scale;
    [Default(0.5)] [SoftRange(0.0, 2.0)] [Group("geometry")] float detail_strength;
    [Texture("data")] [Group("geometry")] RikkaTexture occlusion_texture;
    [Default(1.0)] [Range(0.0, 1.0)] [Group("geometry")] float occlusion_strength;
    [Default(1.0)] [Range(0.0, 1.0)] [Group("geometry")] [Label("Opacity")] float geometry_opacity;
    RikkaUvTransform uv;

    [constref]
    RikkaSurfaceData<RikkaOpenPbrData> evaluate(RikkaPixel pixel, RikkaContext context) {
        float2 at = uv.apply(pixel.uv);
        float4 base = float4(base_color, geometry_opacity) * pixel.color * base_color_texture.sampled(at);
        float rough = roughness_texture.sampled(at).g;
        float cavity = lerp(1.0, cavity_texture.sampled(at).r, cavity_strength);
        RikkaOpenPbrData d = rikka_openpbr_data();
        d.diffuse_roughness = base_diffuse_roughness;
        d.specular_weight = specular_weight;
        d.specular_roughness = specular_roughness * rough;
        d.second_roughness = specular_second_roughness * rough;
        d.second_lobe = specular_lobe_mix;
        d.specular_ior = specular_ior;
        d.subsurface_weight = subsurface_weight;
        d.subsurface_color = clamp(base.rgb * subsurface_tint, float3(0.0), float3(1.0));
        d.subsurface_radius = subsurface_radius * subsurface_radius_scale;
        d.subsurface_anisotropy = subsurface_scatter_anisotropy;
        d.thickness = thickness * thickness_texture.sampled(at).g;
        d.fuzz_weight = fuzz_weight;
        d.fuzz_color = fuzz_color;
        d.fuzz_roughness = fuzz_roughness;
        d.cavity = cavity;
        RikkaSurfaceData<RikkaOpenPbrData> s = rikka_surface_data(pixel, d);
        if (normal_texture.bound() || detail_normal_texture.bound()) {
            float3 tangent_normal = normal_texture.bound()
                ? rikka_pbr_tangent_normal(normal_texture, at, normal_scale)
                : float3(0.0, 0.0, 1.0);
            if (detail_normal_texture.bound()) {
                float3 detail = rikka_unpack_normal(detail_normal_texture.sampled(at * detail_scale));
                tangent_normal = rikka_pbr_detail_normal(tangent_normal, detail, detail_strength);
            }
            s.normal = pixel.normal_map(tangent_normal);
        }
        s.albedo = base.rgb;
        s.alpha = base.a;
        s.occlusion = lerp(1.0, occlusion_texture.sampled(at).r, occlusion_strength) * lerp(1.0, cavity, 0.5);
        return s;
    }

    [constref]
    override float coverage(RikkaCoverage coverage, RikkaContext context) {
        return geometry_opacity * (base_color_texture.bound() ? base_color_texture.sampled(uv.apply(coverage.uv)).a : 1.0);
    }
};
