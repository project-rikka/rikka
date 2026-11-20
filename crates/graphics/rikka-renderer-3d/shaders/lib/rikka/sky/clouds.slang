__exported import rikka.sky.atmosphere;
__exported import rikka.sky.noise;

static const float RIKKA_CLOUD_ALTITUDE = 2.0;
static const float RIKKA_CLOUD_FEATURE = 0.42;
static const float2 RIKKA_CLOUD_WIND = float2(0.012, 0.005);
static const float RIKKA_CLOUD_HAZE_DISTANCE = 90.0;

float rikka_cloud_density(float2 p, float coverage) {
    float2 warp = float2(rikka_fbm(p * 0.45 + float2(3.1, 1.7), 3),
                         rikka_fbm(p * 0.45 - float2(7.7, 2.3), 3));
    float shape = rikka_fbm(p + warp * 1.4, 6);
    float threshold = lerp(0.68, 0.2, coverage);
    float softness = lerp(0.11, 0.3, coverage);
    return clamp((shape - threshold) / softness, 0.0, 1.0);
}

float3 rikka_cloud_light(RikkaSky sky, float3 at, float2 p, float density, float3 direction,
                         float3 to_light, float3 illuminance) {
    if (to_light.y < -0.12 || max(illuminance.r, max(illuminance.g, illuminance.b)) <= 0.0) {
        return float3(0.0);
    }
    float3 incident = illuminance * rikka_transmittance_to(sky, at, to_light);
    float2 toward = normalize(to_light.xz + float2(1e-4)) * (0.1 + 0.3 * (1.0 - abs(to_light.y)));
    float shade = rikka_cloud_density(p + toward, sky.clouds)
        + 0.6 * rikka_cloud_density(p + toward * 2.5, sky.clouds);
    float beer = lerp(exp(-(shade + density) * 1.6), 1.0, 0.25);
    float powder = 1.0 - exp(-density * 5.0);
    float c = dot(direction, to_light);
    float forward = rikka_henyey_greenstein(0.6, c) * (1.0 - 0.65 * density);
    return incident * (beer * (0.26 + 0.12 * powder) + 1.3 * forward * lerp(beer, 1.0, 0.35));
}

float4 rikka_clouds(RikkaSky sky, float3 direction, float time, float3 ambient) {
    if (sky.clouds <= 0.0 || direction.y <= 0.0) {
        return float4(0.0);
    }
    float3 origin = rikka_viewer_origin();
    float t = rikka_ray_sphere(origin, direction, RIKKA_PLANET_RADIUS + RIKKA_CLOUD_ALTITUDE);
    float3 at = origin + direction * t;
    float2 drift = RIKKA_CLOUD_WIND * time * sky.cloud_drift;
    float2 p = (at.xz + drift) * RIKKA_CLOUD_FEATURE;
    float density = rikka_cloud_density(p, sky.clouds);
    if (density <= 0.0) {
        return float4(0.0);
    }
    float3 moon = sky.to_sun.y < 0.1 ? sky.moon_illuminance * sky.moon_illumination : float3(0.0);
    float3 color = ambient * (1.25 - 0.45 * density)
        + rikka_cloud_light(sky, at, p, density, direction, sky.to_sun, sky.sun_illuminance)
        + rikka_cloud_light(sky, at, p, density, direction, sky.to_moon, moon);
    float opacity = clamp(density * 1.7, 0.0, 1.0) * lerp(0.75, 1.0, sky.clouds);
    float fade = exp(-t / RIKKA_CLOUD_HAZE_DISTANCE) * smoothstep(0.0, 0.05, direction.y);
    return float4(color, opacity * fade);
}
