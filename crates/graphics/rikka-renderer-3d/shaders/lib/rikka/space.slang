__exported import rikka.common;

float4 rikka_occlusion(RikkaFrameRef frame, float2 uv) {
    uint image = frame.look.space.occlusion_image;
    if (frame.look.scene.enabled == 0u || image == RIKKA_NONE) {
        return float4(1.0, 0.0, 0.0, 0.0);
    }
    return rikka_images[NonUniformResourceIndex(image)]
        .Sample(rikka_samplers[1], rikka_render_uv(frame.view, uv, 1u));
}

float rikka_screen_shadow(RikkaFrameRef frame, float2 frag_xy) {
    RikkaSpace space = frame.look.space;
    if (frame.look.scene.enabled == 0u || space.screen_shadow_image == RIKKA_NONE) {
        return 1.0;
    }
    int2 texel = min(int2(frag_xy), int2(frame.view.viewport) - 1);
    float shadow = rikka_fetch(rikka_images[NonUniformResourceIndex(space.screen_shadow_image)], texel, 0).r;
    return 1.0 - min(shadow * space.screen_shadow_strength, 1.0);
}

float rikka_reflectance(float reflection, float roughness, float n_dot_v) {
    float f0 = reflection * reflection;
    float grazing = max(1.0 - roughness, f0);
    float fresnel = f0 + (grazing - f0) * pow(clamp(1.0 - n_dot_v, 0.0, 1.0), 5.0);
    return fresnel * clamp(reflection * 10.0, 0.0, 1.0);
}
