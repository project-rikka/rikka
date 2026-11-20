import rikka.push;
import rikka.sky.dome;
import rikka.sky.bodies;
import rikka.sky;

[vk::constant_id(0)] const float HORIZON_SHARPNESS = 8.0;

float3 gradient(float3 direction) {
    float3 above_color = float3(0.36, 0.44, 0.58);
    float3 horizon = float3(0.55, 0.58, 0.62);
    float3 below_color = float3(0.16, 0.16, 0.18);
    float above = pow(clamp(direction.y, 0.0, 1.0), 1.0 / HORIZON_SHARPNESS);
    float below = pow(clamp(-direction.y, 0.0, 1.0), 1.0 / HORIZON_SHARPNESS);
    return lerp(lerp(horizon, above_color, above), below_color, below);
}

[shader("fragment")]
float4 fragment_main([[vk::location(0)]] float2 uv) : SV_Target {
    RikkaFrameRef frame = rikka_push.frame;
    RikkaViewRef view = frame.view;
    float3 direction = rikka_view_ray(view, uv * 2.0 - 1.0);
    RikkaSky sky = frame.look.sky;
    if (frame.look.scene.enabled == 0u || sky.physical == 0u
        || sky.sky_view_lut == RIKKA_NONE) {
        return float4(gradient(direction), 1.0);
    }

    float pixel = max(length(fwidth(direction)), 1e-5);
    float3 ground = rikka_ground_radiance(sky, rikka_sky_irradiance(frame, float3(0.0, 1.0, 0.0)));
    RikkaDome dome = rikka_sky_dome(sky, direction, view.time, rikka_clear_sky_ambient(sky),
                                    ground);
    float3 passed = rikka_transmittance_to(sky, rikka_viewer_origin(), direction);
    float3 bodies = rikka_sun_disc(sky, direction, passed, pixel)
        + rikka_moon_disc(sky, direction, passed, pixel)
        + rikka_night_sky(sky, direction, passed, pixel, view.time);
    float3 glow = rikka_aureoles(sky, direction, passed);
    float3 color = dome.radiance + glow * (1.0 - dome.ground)
        + bodies * ((1.0 - min(dome.clouds * 1.6, 1.0)) * (1.0 - dome.ground));
    return float4(color, 1.0);
}
