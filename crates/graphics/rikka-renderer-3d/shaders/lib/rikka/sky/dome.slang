__exported import rikka.sky.clouds;

struct RikkaDome {
    float3 radiance;
    float transmittance;
    float clouds;
    float ground;
};

float3 rikka_clear_sky_ambient(RikkaSky sky) {
    float3 sum = 2.0 * rikka_sky_view(sky, float3(0.0, 1.0, 0.0)).rgb;
    [unroll]
    for (int i = 0; i < 4; ++i) {
        float azimuth = (float(i) + 0.5) * 0.5 * RIKKA_PI;
        float2 around = float2(cos(azimuth), sin(azimuth));
        sum += rikka_sky_view(sky, float3(around.x * 0.77, 0.64, around.y * 0.77)).rgb;
        sum += rikka_sky_view(sky, float3(around.x * 0.98, 0.2, around.y * 0.98)).rgb;
    }
    return sum * 0.1;
}

RikkaDome rikka_sky_dome(RikkaSky sky, float3 direction, float time, float3 ambient,
                         float3 ground_radiance) {
    float4 view = rikka_sky_view(sky, direction);
    float ground = rikka_ground_cover(direction);
    RikkaDome dome;
    dome.radiance = view.rgb + ground_radiance * view.a * ground;
    dome.transmittance = view.a;
    dome.ground = ground;
    float4 clouds = rikka_clouds(sky, direction, time, ambient);
    dome.radiance = lerp(dome.radiance, clouds.rgb, clouds.a);
    dome.clouds = clouds.a;
    return dome;
}
