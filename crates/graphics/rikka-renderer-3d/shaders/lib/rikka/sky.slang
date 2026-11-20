__exported import rikka.common;
__exported import rikka.sky.environment;

bool rikka_sky_lit(RikkaFrameRef frame) {
    return frame.look.scene.enabled != 0u && frame.look.sky.environment != 0u;
}

float3 rikka_sky_irradiance(RikkaFrameRef frame, float3 n) {
    if (!rikka_sky_lit(frame)) {
        return frame.view.ambient;
    }
    float4 c[9] = frame.environment.irradiance;
    float3 result = c[0].rgb * 0.282095
        + c[1].rgb * (0.488603 * n.y)
        + c[2].rgb * (0.488603 * n.z)
        + c[3].rgb * (0.488603 * n.x)
        + c[4].rgb * (1.092548 * n.x * n.y)
        + c[5].rgb * (1.092548 * n.y * n.z)
        + c[6].rgb * (0.315392 * (3.0 * n.z * n.z - 1.0))
        + c[7].rgb * (1.092548 * n.x * n.z)
        + c[8].rgb * (0.546274 * (n.x * n.x - n.y * n.y));
    return max(result, float3(0.0));
}

float3 rikka_sky_specular(RikkaFrameRef frame, float3 r, float roughness) {
    RikkaSky sky = frame.look.sky;
    if (!rikka_sky_lit(frame) || sky.environment_image == RIKKA_NONE
        || sky.environment_levels == 0u) {
        return rikka_sky_irradiance(frame, r);
    }
    float level = clamp(roughness, 0.0, 1.0) * float(sky.environment_levels - 1u);
    uint lower = uint(floor(level));
    uint upper = min(lower + 1u, sky.environment_levels - 1u);
    float3 direction = normalize(r);
    return lerp(rikka_environment_texel(sky.environment_image, direction, lower),
                rikka_environment_texel(sky.environment_image, direction, upper),
                level - float(lower));
}
