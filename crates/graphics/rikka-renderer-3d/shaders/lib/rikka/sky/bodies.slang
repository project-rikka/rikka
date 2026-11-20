__exported import rikka.sky.atmosphere;
__exported import rikka.sky.noise;

static const float RIKKA_SUN_DISC_GAIN = 30.0;
static const float RIKKA_MOON_DISC_GAIN = 60.0;
static const float RIKKA_AUREOLE = 0.01;
static const float RIKKA_EARTHSHINE = 0.012;
static const float RIKKA_STAR_CELLS = 360.0;
static const float RIKKA_STAR_CHANCE = 0.022;
static const float RIKKA_STAR_BRIGHTNESS = 0.008;
static const float RIKKA_MILKY_WAY = 0.006;
static const float3 RIKKA_GALACTIC_POLE = float3(-0.868, -0.198, 0.456);

float rikka_angle_between(float3 a, float3 b) {
    return 2.0 * asin(clamp(0.5 * length(a - b), 0.0, 1.0));
}

float rikka_disc_cover(float angle, float radius, float pixel) {
    return smoothstep(radius + pixel, radius - pixel, angle);
}

float3 rikka_sun_disc(RikkaSky sky, float3 direction, float3 passed, float pixel) {
    float radius = sky.sun_angular_radius;
    float angle = rikka_angle_between(direction, sky.to_sun);
    if (radius <= 0.0 || angle > radius + 2.0 * pixel) {
        return float3(0.0);
    }
    float x = clamp(angle / radius, 0.0, 1.0);
    float mu = sqrt(1.0 - x * x);
    float limb = 1.0 - 0.6 * (1.0 - mu) - 0.15 * (1.0 - mu) * (1.0 - mu);
    return sky.sun_illuminance * passed * (RIKKA_SUN_DISC_GAIN * limb)
        * rikka_disc_cover(angle, radius, pixel);
}

float3 rikka_aureoles(RikkaSky sky, float3 direction, float3 passed) {
    float sun = rikka_henyey_greenstein(0.97, dot(direction, sky.to_sun));
    float moon = rikka_henyey_greenstein(0.97, dot(direction, sky.to_moon));
    float3 glow = sky.sun_illuminance * sun
        + sky.moon_illuminance * (sky.moon_illumination * moon);
    return glow * passed * (RIKKA_AUREOLE * sky.haze);
}

float3 rikka_moon_disc(RikkaSky sky, float3 direction, float3 passed, float pixel) {
    float radius = sky.moon_angular_radius;
    float angle = rikka_angle_between(direction, sky.to_moon);
    if (radius <= 0.0 || angle > radius + 2.0 * pixel) {
        return float3(0.0);
    }
    float3 helper = abs(sky.to_moon.y) < 0.99 ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0);
    float3 right = normalize(cross(sky.to_moon, helper));
    float3 up = cross(right, sky.to_moon);
    float2 offset = float2(dot(direction, right), dot(direction, up)) / radius;
    float rim = dot(offset, offset);
    float3 normal = right * offset.x + up * offset.y - sky.to_moon * sqrt(max(1.0 - rim, 0.0));
    float incident = max(dot(normal, sky.to_sun), 0.0);
    float emergent = max(dot(normal, -sky.to_moon), 0.0);
    float lit = 2.0 * incident / max(incident + emergent, 1e-3);
    float maria = smoothstep(0.38, 0.62, rikka_fbm(offset * 2.3 + float2(4.7, 1.3), 4));
    float craters = rikka_fbm(offset * 9.0 + float2(-2.1, 7.9), 3);
    float albedo = lerp(0.55, 1.0, maria) * lerp(0.85, 1.1, craters);
    return sky.moon_illuminance * passed * (RIKKA_MOON_DISC_GAIN * albedo
        * (lit + RIKKA_EARTHSHINE)) * rikka_disc_cover(angle, radius, pixel);
}

float3 rikka_star_field(RikkaSky sky, float3 direction, float pixel, float time) {
    float3 c = mul(sky.star_frame, direction);
    float3 a = abs(c);
    uint face;
    float2 uv;
    float major;
    if (a.x >= a.y && a.x >= a.z) {
        face = c.x > 0.0 ? 0u : 1u;
        uv = c.yz;
        major = a.x;
    } else if (a.y >= a.z) {
        face = c.y > 0.0 ? 2u : 3u;
        uv = c.xz;
        major = a.y;
    } else {
        face = c.z > 0.0 ? 4u : 5u;
        uv = c.xy;
        major = a.z;
    }
    float2 at = (uv / major * 0.5 + 0.5) * RIKKA_STAR_CELLS;
    float2 cell = floor(at);
    uint seed = rikka_hash3(uint3(uint2(int2(cell)), face));
    float3 light = float3(0.0);
    if (rikka_unit(seed) < RIKKA_STAR_CHANCE) {
        float2 centre = 0.2 + 0.6 * float2(rikka_unit(rikka_hash(seed + 1u)),
                                           rikka_unit(rikka_hash(seed + 2u)));
        float magnitude = rikka_unit(rikka_hash(seed + 3u));
        float warmth = rikka_unit(rikka_hash(seed + 4u));
        float rate = rikka_unit(rikka_hash(seed + 5u));
        float cell_angle = 2.0 / (RIKKA_STAR_CELLS * major);
        float apart = length(at - cell - centre) * cell_angle;
        float spread = max(pixel * 0.75, 1e-5);
        float shape = exp(-0.5 * apart * apart / (spread * spread));
        float brightness = RIKKA_STAR_BRIGHTNESS * exp(6.0 * pow(magnitude, 6.0));
        float twinkle = 1.0 + (0.25 + 0.35 * (1.0 - clamp(direction.y * 3.0, 0.0, 1.0)))
            * sin(time * (2.0 + 4.0 * rate) + 40.0 * rate);
        float3 tint = lerp(float3(0.68, 0.78, 1.0), float3(1.0, 0.86, 0.66), warmth);
        light = tint * (brightness * shape * twinkle);
    }
    float across = dot(c, RIKKA_GALACTIC_POLE);
    float band = exp(-across * across / 0.03);
    if (band > 0.01) {
        float3 helper = normalize(cross(RIKKA_GALACTIC_POLE, float3(0.0, 0.0, 1.0)));
        float2 plane = float2(dot(c, helper), dot(c, cross(RIKKA_GALACTIC_POLE, helper)));
        float glow = 0.35 + 0.65 * rikka_fbm(plane * 7.0 + across * 9.0, 5);
        float dust = smoothstep(0.5, 0.68, rikka_fbm(plane * 13.0 + float2(3.3, -1.1), 4))
            * exp(-across * across / 0.004);
        float3 tint = lerp(float3(0.55, 0.62, 0.85), float3(0.95, 0.82, 0.7),
                           exp(-across * across / 0.006));
        light += tint * (RIKKA_MILKY_WAY * band * glow * (1.0 - 0.7 * dust));
    }
    return light;
}

float3 rikka_night_sky(RikkaSky sky, float3 direction, float3 passed, float pixel, float time) {
    float night = smoothstep(-0.03, -0.2, sky.to_sun.y);
    if (night <= 0.0 || sky.stars <= 0.0) {
        return float3(0.0);
    }
    float rising = smoothstep(0.0, 0.18, direction.y);
    return rikka_star_field(sky, direction, pixel, time) * passed * (night * rising * sky.stars);
}
