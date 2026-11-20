import rikka.post;

struct Push {
    uint source;
    uint destination;
    uint exposure;
    float exposure_value;
    uint depth;
    uint pad;
    uint2 size;
    uint2 depth_size;
    float2 sun_uv;
    float aspect;
    float3 sun;
    float blades;
    float rotation;
    float sun_radius;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int GHOSTS = 6;
static const int SUN_GHOSTS = 6;
static const int SUN_TAPS = 16;
static const float GHOST_THRESHOLD = 3.0;
static const float GHOST_GAIN = 0.3;
static const float HALO_GAIN = 0.15;

static const float3 COATINGS[GHOSTS] = {
    float3(1.0, 0.72, 0.42),
    float3(0.42, 0.78, 1.0),
    float3(0.78, 0.5, 1.0),
    float3(0.5, 1.0, 0.68),
    float3(1.0, 0.52, 0.62),
    float3(0.58, 0.68, 1.0)
};

static const float SUN_GHOST_PLACES[SUN_GHOSTS] = { 0.28, 0.46, 0.62, 0.84, 1.08, 1.32 };
static const float SUN_GHOST_SIZES[SUN_GHOSTS] = { 0.025, 0.055, 0.018, 0.085, 0.04, 0.12 };

float3 bright(float2 uv) {
    if (any(uv < float2(0.0)) || any(uv > float2(1.0))) {
        return float3(0.0);
    }
    float4 color = post_sample(push.source, uv);
    return max(color.rgb - float3(GHOST_THRESHOLD), float3(0.0));
}

float3 chromatic(float2 uv, float2 direction, float spread) {
    return float3(
        bright(uv + direction * spread).r,
        bright(uv).g,
        bright(uv - direction * spread).b);
}

float edge_falloff(float2 uv, float power) {
    return pow(1.0 - clamp(length(float2(0.5) - uv) / 0.7071, 0.0, 1.0), power);
}

float3 lens_ghosts(float2 uv) {
    float2 flipped = float2(1.0) - uv;
    float2 towards = float2(0.5) - flipped;
    float2 ghost_step = towards * 0.37;
    float2 direction = normalize(towards + float2(1e-5));
    float3 sum = float3(0.0);
    for (int i = 0; i < GHOSTS; ++i) {
        float2 at = flipped + ghost_step * float(i);
        float weight = edge_falloff(at, 6.0);
        sum += chromatic(at, direction, 0.004 * float(i + 1)) * weight * COATINGS[i] * GHOST_GAIN;
    }
    float2 aspect = float2(push.aspect, 1.0);
    float2 halo = normalize(towards * aspect + float2(1e-5)) / aspect * 0.45;
    float2 at = flipped + halo;
    float ring = edge_falloff(at, 5.0);
    sum += chromatic(at, direction, 0.012) * ring * float3(0.95, 0.85, 1.0) * HALO_GAIN;
    return sum;
}

float sun_visibility() {
    float visible = 0.0;
    float2 scale = float2(1.0 / push.aspect, 1.0) * push.sun_radius;
    for (int tap = 0; tap < SUN_TAPS; ++tap) {
        float radius = sqrt((float(tap) + 0.5) / float(SUN_TAPS));
        float angle = float(tap) * 2.39996323;
        float2 at = push.sun_uv + radius * float2(cos(angle), sin(angle)) * scale;
        if (any(at < float2(0.0)) || any(at >= float2(1.0))) {
            continue;
        }
        int2 texel = int2(at * float2(push.depth_size));
        visible += post_fetch(push.depth, texel).r <= 0.0 ? 1.0 : 0.0;
    }
    return visible / float(SUN_TAPS);
}

float fine_rays(float angle) {
    float position = (angle + RIKKA_PI) * 36.0 / RIKKA_PI;
    float cell = floor(position);
    float blend = smoothstep(0.0, 1.0, fract(position));
    float a = post_random(uint3(uint(cell) % 72u, 11u, 5u));
    float b = post_random(uint3(uint(cell + 1.0) % 72u, 11u, 5u));
    return pow(lerp(a, b, blend), 5.0);
}

float3 sun_flare(float2 uv) {
    float visibility = sun_visibility();
    if (visibility <= 0.0) {
        return float3(0.0);
    }
    float2 aspect = float2(push.aspect, 1.0);
    float2 offset = (uv - push.sun_uv) * aspect;
    float distance = length(offset);
    float angle = atan2(offset.y, offset.x);
    float blades = push.blades;
    float spikes = blades >= 3.0 ? (rikka_mod(blades, 2.0) == 0.0 ? blades : 2.0 * blades) : 6.0;
    float rays = pow(abs(cos(0.5 * spikes * (angle - push.rotation))), 90.0);
    float star = (rays + 0.3 * fine_rays(angle)) * 0.02 / (distance + 0.02) * exp(-distance * 2.5);
    float core = 0.06 / (1.0 + distance * distance * 3000.0);
    float3 color = float3(star + core);

    float2 axis = float2(0.5) - push.sun_uv;
    for (int k = 0; k < SUN_GHOSTS; ++k) {
        float2 place = push.sun_uv + 2.0 * axis * SUN_GHOST_PLACES[k];
        float2 local = (uv - place) * aspect;
        float size = SUN_GHOST_SIZES[k];
        float shaped = length(local) / post_polygon(atan2(local.y, local.x), blades, push.rotation);
        float disc = smoothstep(size, size * 0.82, shaped) * (0.55 + 0.45 * smoothstep(size * 0.4, size, shaped));
        color += disc * COATINGS[k] * 0.01;
    }
    return color * push.sun * visibility;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 texel = int2(global_id.xy);
    if (any(texel >= int2(push.size))) {
        return;
    }
    float2 uv = (float2(texel) + 0.5) / float2(push.size);
    float3 flare = lens_ghosts(uv);
    if (dot(push.sun, push.sun) > 0.0) {
        flare += sun_flare(uv) * post_exposure_or(push.exposure, push.exposure_value);
    }
    post_store(push.destination, texel, float4(flare, 0.0));
}
