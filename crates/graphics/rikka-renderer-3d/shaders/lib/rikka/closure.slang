__exported import rikka.material;
__exported import rikka.standard;
__exported import rikka.pbr.openpbr;
__exported import rikka.pbr.hair;
__exported import rikka.pbr.eye;
__exported import rikka.npr;
__exported import rikka.closures.diffuse;
__exported import rikka.closures.glossy;
__exported import rikka.closures.sheen;
__exported import rikka.closures.translucent;
__exported import rikka.closures.toon;

static const float RIKKA_SHADE_EPSILON = 1e-4;

struct RikkaShade {
    float3 color;
    float alpha;
};

RikkaShade rikka_shade(float3 color, float alpha) {
    RikkaShade shade;
    shade.color = color * alpha;
    shade.alpha = alpha;
    return shade;
}

RikkaShade rikka_shade_mix(RikkaShade a, RikkaShade b, float factor) {
    float t = saturate(factor);
    RikkaShade shade;
    shade.color = lerp(a.color, b.color, t);
    shade.alpha = lerp(a.alpha, b.alpha, t);
    return shade;
}

RikkaShade rikka_shade_add(RikkaShade a, RikkaShade b) {
    RikkaShade shade;
    shade.color = a.color + b.color;
    shade.alpha = saturate(a.alpha + b.alpha);
    return shade;
}

float4 rikka_shade_straight(RikkaShade shade) {
    return float4(shade.color / max(shade.alpha, RIKKA_SHADE_EPSILON), shade.alpha);
}

RikkaTexture rikka_graph_no_texture() {
    RikkaTexture texture;
    texture.image = RIKKA_NONE;
    texture.sampler = 0u;
    return texture;
}

RikkaStandardData rikka_closure_standard_data() {
    RikkaStandardData data;
    data.metallic = 0.0;
    data.roughness = 0.5;
    return data;
}

RikkaHairData rikka_closure_hair_data() {
    RikkaHairData data;
    data.tangent = float3(0.0, 1.0, 0.0);
    data.roughness = 0.35;
    data.shift = 0.035;
    data.specular = 1.0;
    data.backlit = 1.0;
    data.scatter = 0.4;
    return data;
}

RikkaEyeData rikka_closure_eye_data() {
    RikkaEyeData data;
    data.forward = float3(0.0, 0.0, 1.0);
    data.iris = 0.0;
    data.cornea_roughness = 0.04;
    data.cornea_ior = 1.376;
    data.caustic = 2.0;
    data.sclera_scatter = 0.0;
    return data;
}

RikkaOpenPbrData rikka_closure_skin_data() {
    RikkaOpenPbrData data = rikka_openpbr_data();
    data.subsurface_weight = 1.0;
    data.subsurface_color = float3(0.8, 0.6, 0.5);
    data.subsurface_radius = float3(0.004, 0.00148, 0.00076);
    data.specular_roughness = 0.5;
    data.second_roughness = 0.3;
    data.second_lobe = 0.15;
    data.specular_ior = 1.4;
    return data;
}

M.State rikka_closure_prepare<M : IShadingModel>(RikkaPixel pixel, RikkaSurfaceData<M.Data> surface, RikkaContext context) {
    M model;
    return model.prepare(pixel, surface, context);
}

void rikka_closure_direct<M : IShadingModel>(inout M.State state, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
    M model;
    model.direct(state, light, shadow, context);
}

void rikka_closure_transmit<M : IShadingModel>(inout M.State state, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
    M model;
    model.transmit(state, light, shadow, context);
}

void rikka_closure_indirect<M : IShadingModel>(inout M.State state, RikkaIndirect indirect, RikkaContext context) {
    M model;
    model.indirect(state, indirect, context);
}

RikkaShade rikka_closure_shade<M : IShadingModel>(M.State state, RikkaPixel pixel, RikkaContext context) {
    M model;
    RikkaContext open = context;
    open.pass &= ~RIKKA_PASS_MASKED;
    float4 lit = model.resolve(state, pixel, open);
    return rikka_shade(lit.rgb, lit.a);
}

RikkaShade rikka_closure_lit<M : IShadingModel>(RikkaPixel pixel, RikkaSurfaceData<M.Data> surface, bool live, RikkaContext context) {
    M model;
    M.State state = model.prepare(pixel, surface, context);
    if (!live) {
        return rikka_shade(float3(0.0), 1.0);
    }
    rikka_illuminate(model, state, pixel, context);
    return rikka_closure_shade<M>(state, pixel, context);
}

RikkaScatter rikka_closure_scattered<M : IShadingModel>(RikkaPixel pixel, RikkaSurfaceData<M.Data> surface, RikkaContext context) {
    M model;
    return model.scattered(pixel, surface, context);
}

void rikka_scatter_add(inout RikkaScatter total, RikkaScatter part, float weight) {
    float3 added = part.diffuse.rgb * weight;
    float before = rikka_luma(total.diffuse.rgb);
    float share = rikka_luma(added) / max(before + rikka_luma(added), RIKKA_SHADE_EPSILON);
    total.diffuse = float4(total.diffuse.rgb + added, max(total.diffuse.a, part.diffuse.a));
    total.profile = float4(lerp(total.profile.rgb, part.profile.rgb, share), max(total.profile.a, part.profile.a));
}
