__exported import rikka.ray;
__exported import rikka.denoise;

static const float RIKKA_BOUNCE_BRIGHTEST = 64.0;
static const float RIKKA_BOUNCE_REUSE_TOLERANCE = 0.05;
static const float RIKKA_BOUNCE_SKY_ROUGHNESS = 1.0;

struct RikkaBounce {
    uint radiance;
    uint guide_history;
    float reach;
};

struct RikkaBounceSample {
    bool sky;
    float distance;
    float3 position;
    float3 normal;
    float3 radiance;
};

float3 rikka_bounce_reused(RikkaFrameRef frame, RikkaBounce bounce, float3 position, float3 normal, out bool found) {
    found = false;
    if (bounce.radiance == RIKKA_NONE || frame.view.cut != 0u) {
        return float3(0.0);
    }
    float4 clip = mul(frame.look.scene.previous_view_projection, float4(position, 1.0));
    if (clip.w <= 0.0) {
        return float3(0.0);
    }
    float2 uv = rikka_previous_image_uv(frame.view, clip.xy / clip.w * 0.5 + 0.5);
    if (any(uv < float2(0.0)) || any(uv >= float2(1.0))) {
        return float3(0.0);
    }
    int2 radiance_size = int2(rikka_previous_size(frame.view, 2u));
    int2 texel = min(int2(uv * float2(radiance_size)), radiance_size - 1);
    float4 guide = rikka_fetch(rikka_images[bounce.guide_history], texel, 0);
    if (guide.w <= 0.0 || abs(guide.w - clip.w) > RIKKA_BOUNCE_REUSE_TOLERANCE * clip.w
        || dot(guide.xyz, normal) < 0.5) {
        return float3(0.0);
    }
    found = true;
    return rikka_fetch(rikka_images[bounce.radiance], texel, 0).rgb;
}

RikkaBounceSample rikka_bounce_trace(RikkaFrameRef frame, RikkaBounce bounce, float3 origin, float3 direction,
    float pick, float2 noise) {
    RikkaRayHit hit = rikka_ray_closest(frame, origin, direction, bounce.reach);
    RikkaBounceSample s;
    s.sky = !hit.hit;
    s.distance = hit.distance;
    if (!hit.hit) {
        s.position = direction;
        s.normal = -direction;
        s.radiance = rikka_sky_specular(frame, direction, RIKKA_BOUNCE_SKY_ROUGHNESS);
        return s;
    }
    s.position = hit.position;
    s.normal = hit.normal;
    bool found;
    float3 again = rikka_bounce_reused(frame, bounce, hit.position, hit.normal, found);
    s.radiance = found ? again : rikka_ray_shade(frame, hit, pick, noise, bounce.reach);
    return s;
}

float3 rikka_bounce_bounded(float3 radiance) {
    float3 bounded = min(radiance, float3(RIKKA_BOUNCE_BRIGHTEST));
    return any(isnan(bounded)) ? float3(0.0) : bounded;
}
