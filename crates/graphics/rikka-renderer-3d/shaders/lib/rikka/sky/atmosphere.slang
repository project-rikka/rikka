__exported import rikka.common;

static const uint RIKKA_ATMO_SAMPLER_REPEAT = 0u;
static const uint RIKKA_ATMO_SAMPLER_CLAMP = 1u;

static const float RIKKA_PLANET_RADIUS = 6360.0;
static const float RIKKA_ATMOSPHERE_RADIUS = 6460.0;
static const float3 RIKKA_RAYLEIGH_SCATTERING = float3(5.802e-3, 13.558e-3, 33.1e-3);
static const float RIKKA_RAYLEIGH_HEIGHT = 8.0;
static const float RIKKA_MIE_SCATTERING = 3.996e-3;
static const float RIKKA_MIE_EXTINCTION = 4.44e-3;
static const float RIKKA_MIE_HEIGHT = 1.2;
static const float3 RIKKA_OZONE_ABSORPTION = float3(0.650e-3, 1.881e-3, 0.085e-3);
static const float RIKKA_OZONE_CENTER = 25.0;
static const float RIKKA_OZONE_WIDTH = 15.0;
static const float RIKKA_VIEWER_ALTITUDE = 0.2;
static const float RIKKA_MIE_ANISOTROPY = 0.8;
static const float RIKKA_MOONLIT_SKY = 0.4;
static const float3 RIKKA_AIRGLOW = float3(0.0011, 0.0016, 0.0034);
static const float RIKKA_WORLD_TO_KM = 0.001;

struct RikkaMedium {
    float3 rayleigh;
    float mie;
    float3 scattering;
    float3 extinction;
};

float3 rikka_viewer_origin() {
    return float3(0.0, RIKKA_PLANET_RADIUS + RIKKA_VIEWER_ALTITUDE, 0.0);
}

RikkaMedium rikka_medium(float altitude, float haze) {
    float rayleigh = exp(-altitude / RIKKA_RAYLEIGH_HEIGHT);
    float mie = exp(-altitude / RIKKA_MIE_HEIGHT) * haze;
    float ozone = max(0.0, 1.0 - abs(altitude - RIKKA_OZONE_CENTER) / RIKKA_OZONE_WIDTH);
    RikkaMedium medium;
    medium.rayleigh = RIKKA_RAYLEIGH_SCATTERING * rayleigh;
    medium.mie = RIKKA_MIE_SCATTERING * mie;
    medium.scattering = medium.rayleigh + float3(medium.mie);
    medium.extinction = medium.rayleigh + float3(RIKKA_MIE_EXTINCTION * mie)
        + RIKKA_OZONE_ABSORPTION * ozone;
    return medium;
}

float rikka_ray_sphere(float3 origin, float3 direction, float radius) {
    float b = dot(origin, direction);
    float c = dot(origin, origin) - radius * radius;
    float discriminant = b * b - c;
    if (discriminant < 0.0) {
        return -1.0;
    }
    float root = sqrt(discriminant);
    float near = -b - root;
    return near >= 0.0 ? near : -b + root;
}

bool rikka_hits_ground(float3 origin, float3 direction) {
    return rikka_ray_sphere(origin, direction, RIKKA_PLANET_RADIUS) > 0.0
        && dot(origin, direction) < 0.0;
}

float rikka_atmosphere_path(float3 origin, float3 direction) {
    if (rikka_hits_ground(origin, direction)) {
        return rikka_ray_sphere(origin, direction, RIKKA_PLANET_RADIUS);
    }
    return max(rikka_ray_sphere(origin, direction, RIKKA_ATMOSPHERE_RADIUS), 0.0);
}

float rikka_rayleigh_phase(float c) {
    return 3.0 / (16.0 * RIKKA_PI) * (1.0 + c * c);
}

float rikka_cornette_shanks(float g, float c) {
    float g2 = g * g;
    float k = 3.0 / (8.0 * RIKKA_PI) * (1.0 - g2) / (2.0 + g2);
    return k * (1.0 + c * c) / pow(max(1.0 + g2 - 2.0 * g * c, 1e-5), 1.5);
}

float rikka_henyey_greenstein(float g, float c) {
    float g2 = g * g;
    return (1.0 - g2) / (4.0 * RIKKA_PI * pow(max(1.0 + g2 - 2.0 * g * c, 1e-5), 1.5));
}

float4 rikka_atmo_fetch(uint image, float2 uv, uint sampler_slot) {
    return rikka_images[image].SampleLevel(rikka_samplers[sampler_slot], uv, 0.0);
}

float2 rikka_lut_size(uint image) {
    return float2(rikka_image_size(rikka_images[image], 0));
}

float rikka_to_sub_uv(float unit, float size) {
    return (unit * (size - 1.0) + 0.5) / size;
}

float rikka_from_sub_uv(float uv, float size) {
    return (uv * size - 0.5) / (size - 1.0);
}

float2 rikka_transmittance_uv(float r, float mu, float2 size) {
    float top2 = RIKKA_ATMOSPHERE_RADIUS * RIKKA_ATMOSPHERE_RADIUS;
    float bottom2 = RIKKA_PLANET_RADIUS * RIKKA_PLANET_RADIUS;
    float h = sqrt(top2 - bottom2);
    float rho = sqrt(max(r * r - bottom2, 0.0));
    float discriminant = r * r * (mu * mu - 1.0) + top2;
    float d = max(0.0, -r * mu + sqrt(max(discriminant, 0.0)));
    float d_min = RIKKA_ATMOSPHERE_RADIUS - r;
    float d_max = rho + h;
    float x_mu = (d - d_min) / max(d_max - d_min, 1e-4);
    float x_r = rho / h;
    return float2(rikka_to_sub_uv(clamp(x_mu, 0.0, 1.0), size.x),
                  rikka_to_sub_uv(clamp(x_r, 0.0, 1.0), size.y));
}

void rikka_transmittance_params(float2 uv, float2 size, out float r, out float mu) {
    float x_mu = clamp(rikka_from_sub_uv(uv.x, size.x), 0.0, 1.0);
    float x_r = clamp(rikka_from_sub_uv(uv.y, size.y), 0.0, 1.0);
    float top2 = RIKKA_ATMOSPHERE_RADIUS * RIKKA_ATMOSPHERE_RADIUS;
    float bottom2 = RIKKA_PLANET_RADIUS * RIKKA_PLANET_RADIUS;
    float h = sqrt(top2 - bottom2);
    float rho = h * x_r;
    r = sqrt(rho * rho + bottom2);
    float d_min = RIKKA_ATMOSPHERE_RADIUS - r;
    float d_max = rho + h;
    float d = d_min + x_mu * (d_max - d_min);
    mu = d == 0.0 ? 1.0 : (h * h - rho * rho - d * d) / (2.0 * r * d);
    mu = clamp(mu, -1.0, 1.0);
}

float3 rikka_transmittance_to(RikkaSky sky, float3 position, float3 direction) {
    if (rikka_hits_ground(position, direction)) {
        return float3(0.0);
    }
    float r = max(length(position), RIKKA_PLANET_RADIUS + 1e-3);
    float mu = dot(position, direction) / length(position);
    float2 size = rikka_lut_size(sky.transmittance_lut);
    return rikka_atmo_fetch(sky.transmittance_lut, rikka_transmittance_uv(r, mu, size),
                            RIKKA_ATMO_SAMPLER_CLAMP).rgb;
}

float3 rikka_multiscatter(RikkaSky sky, float altitude, float sun_mu) {
    float2 size = rikka_lut_size(sky.multiscatter_lut);
    float2 unit = float2(sun_mu * 0.5 + 0.5,
                         altitude / (RIKKA_ATMOSPHERE_RADIUS - RIKKA_PLANET_RADIUS));
    float2 uv = float2(rikka_to_sub_uv(clamp(unit.x, 0.0, 1.0), size.x),
                       rikka_to_sub_uv(clamp(unit.y, 0.0, 1.0), size.y));
    return rikka_atmo_fetch(sky.multiscatter_lut, uv, RIKKA_ATMO_SAMPLER_CLAMP).rgb;
}

float3 rikka_moonlight(RikkaSky sky) {
    return sky.moon_illuminance * sky.moon_illumination * RIKKA_MOONLIT_SKY;
}

float3 rikka_body_scattering(RikkaSky sky, float3 position, RikkaMedium medium, float3 view,
                             float3 to_body, float3 illuminance) {
    float c = dot(view, to_body);
    float3 phased = medium.rayleigh * rikka_rayleigh_phase(c)
        + float3(medium.mie * rikka_cornette_shanks(RIKKA_MIE_ANISOTROPY, c));
    float3 direct = rikka_transmittance_to(sky, position, to_body) * phased;
    float r = length(position);
    float3 multiple = rikka_multiscatter(sky, r - RIKKA_PLANET_RADIUS, dot(position, to_body) / r)
        * medium.scattering;
    return illuminance * (direct + multiple);
}

float3 rikka_atmosphere_light(RikkaSky sky, float3 position, RikkaMedium medium, float3 view) {
    float3 light = rikka_body_scattering(sky, position, medium, view, sky.to_sun,
                                         sky.sun_illuminance);
    float3 moon = rikka_moonlight(sky);
    if (max(moon.r, max(moon.g, moon.b)) > 0.0) {
        light += rikka_body_scattering(sky, position, medium, view, sky.to_moon, moon);
    }
    return light;
}

void rikka_scatter_step(float3 source, float3 extinction, float step, inout float3 light,
                        inout float3 throughput) {
    float3 passed = exp(-extinction * step);
    float3 integrated = (source - source * passed) / max(extinction, float3(1e-7));
    light += throughput * integrated;
    throughput *= passed;
}

float2 rikka_sky_view_uv(float3 direction) {
    float azimuth = atan2(direction.z, direction.x);
    float elevation = asin(clamp(direction.y, -1.0, 1.0));
    float v = 0.5 + 0.5 * sign(elevation) * sqrt(abs(elevation) / (0.5 * RIKKA_PI));
    return float2(azimuth / (2.0 * RIKKA_PI) + 0.5, v);
}

float3 rikka_sky_view_direction(float2 uv) {
    float azimuth = (uv.x - 0.5) * 2.0 * RIKKA_PI;
    float signed_v = (uv.y - 0.5) * 2.0;
    float elevation = sign(signed_v) * signed_v * signed_v * 0.5 * RIKKA_PI;
    float c = cos(elevation);
    return float3(c * cos(azimuth), sin(elevation), c * sin(azimuth));
}

float4 rikka_sky_view(RikkaSky sky, float3 direction) {
    float2 size = rikka_lut_size(sky.sky_view_lut);
    float2 uv = rikka_sky_view_uv(direction);
    uv.y = clamp(uv.y, 0.5 / size.y, 1.0 - 0.5 / size.y);
    return rikka_atmo_fetch(sky.sky_view_lut, uv, RIKKA_ATMO_SAMPLER_REPEAT);
}

float rikka_ground_cover(float3 direction) {
    float dip = -sqrt(max(1.0 - pow(RIKKA_PLANET_RADIUS
        / (RIKKA_PLANET_RADIUS + RIKKA_VIEWER_ALTITUDE), 2.0), 0.0));
    return smoothstep(dip + 0.0015, dip - 0.0015, direction.y);
}

float3 rikka_ground_radiance(RikkaSky sky, float3 sky_irradiance_over_pi) {
    float3 direct = sky.sun_color * max(sky.to_sun.y, 0.0)
        + sky.moon_color * max(sky.to_moon.y, 0.0);
    return sky.ground * (direct / RIKKA_PI + sky_irradiance_over_pi);
}
