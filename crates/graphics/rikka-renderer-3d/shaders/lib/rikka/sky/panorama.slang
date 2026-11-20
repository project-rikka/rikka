__exported import rikka.sky;

static const uint RIKKA_PANORAMA_WIDTH = 512u;
static const uint RIKKA_PANORAMA_HEIGHT = 256u;
static const uint RIKKA_PANORAMA_LEVELS = 8u;
static const float RIKKA_HALF_LARGEST = 65504.0;

float2 rikka_panorama_uv(float3 direction) {
    float u = 0.5 + atan2(direction.x, -direction.z) * (0.5 * RIKKA_INV_PI);
    float v = acos(clamp(direction.y, -1.0, 1.0)) * RIKKA_INV_PI;
    return float2(u, v);
}

float3 rikka_panorama_direction(float2 uv) {
    float azimuth = (uv.x - 0.5) * RIKKA_TAU;
    float polar = uv.y * RIKKA_PI;
    float around = sin(polar);
    return float3(around * sin(azimuth), cos(polar), -around * cos(azimuth));
}

float3 rikka_panorama_from_world(RikkaSky sky, float3 direction) {
    float2 turn = sky.panorama_rotation;
    return float3(turn.x * direction.x - turn.y * direction.z, direction.y,
                  turn.y * direction.x + turn.x * direction.z);
}

float3 rikka_panorama_to_world(RikkaSky sky, float3 direction) {
    float2 turn = sky.panorama_rotation;
    return float3(turn.x * direction.x + turn.y * direction.z, direction.y,
                  turn.x * direction.z - turn.y * direction.x);
}

uint4 rikka_panorama_level(uint level) {
    uint width = RIKKA_PANORAMA_WIDTH >> level;
    uint height = RIKKA_PANORAMA_HEIGHT >> level;
    if (level == 0u) {
        return uint4(0u, 0u, width, height);
    }
    return uint4(RIKKA_PANORAMA_WIDTH - 2u * width, RIKKA_PANORAMA_HEIGHT, width, height);
}

float rikka_panorama_texel_solid_angle(float polar) {
    return RIKKA_TAU / float(RIKKA_PANORAMA_WIDTH) * RIKKA_PI / float(RIKKA_PANORAMA_HEIGHT)
        * max(sin(polar), 1e-4);
}

float3 rikka_panorama_bilinear(Texture2D image, int4 rect, float2 uv) {
    float2 at = uv * float2(rect.zw) - 0.5;
    float2 corner = floor(at);
    float2 blend = at - corner;
    int2 low = int2(corner);
    int x0 = (low.x % rect.z + rect.z) % rect.z;
    int x1 = (x0 + 1) % rect.z;
    int y0 = clamp(low.y, 0, rect.w - 1);
    int y1 = clamp(low.y + 1, 0, rect.w - 1);
    float3 a = rikka_fetch(image, rect.xy + int2(x0, y0), 0).rgb;
    float3 b = rikka_fetch(image, rect.xy + int2(x1, y0), 0).rgb;
    float3 c = rikka_fetch(image, rect.xy + int2(x0, y1), 0).rgb;
    float3 d = rikka_fetch(image, rect.xy + int2(x1, y1), 0).rgb;
    return lerp(lerp(a, b, blend.x), lerp(c, d, blend.x), blend.y);
}

float3 rikka_panorama_sample(uint atlas, float3 direction, float lod) {
    float level = clamp(lod, 0.0, float(RIKKA_PANORAMA_LEVELS - 1u));
    uint lower = uint(floor(level));
    uint upper = min(lower + 1u, RIKKA_PANORAMA_LEVELS - 1u);
    float2 uv = rikka_panorama_uv(direction);
    Texture2D image = rikka_images[NonUniformResourceIndex(atlas)];
    float3 near = rikka_panorama_bilinear(image, int4(rikka_panorama_level(lower)), uv);
    if (upper == lower) {
        return near;
    }
    float3 far = rikka_panorama_bilinear(image, int4(rikka_panorama_level(upper)), uv);
    return lerp(near, far, level - float(lower));
}

float3 rikka_panorama_source(RikkaSky sky, float3 direction) {
    Texture2D image = rikka_images[NonUniformResourceIndex(sky.panorama)];
    int2 size = int2(rikka_image_size(image, 0));
    return rikka_panorama_bilinear(image, int4(0, 0, size), rikka_panorama_uv(direction))
        * sky.panorama_scale;
}

float3 rikka_panorama_background(RikkaFrameRef frame, float3 direction) {
    RikkaSky sky = frame.look.sky;
    float3 sharp = rikka_panorama_source(sky, rikka_panorama_from_world(sky, direction));
    if (sky.panorama_blur <= 0.0 || sky.environment_image == RIKKA_NONE
        || sky.environment_levels < 2u) {
        return sharp;
    }
    float level = sky.panorama_blur * float(sky.environment_levels - 1u);
    uint lower = uint(floor(level));
    uint upper = min(lower + 1u, sky.environment_levels - 1u);
    float3 near = lower == 0u ? sharp
        : rikka_environment_texel(sky.environment_image, direction, lower);
    float3 far = rikka_environment_texel(sky.environment_image, direction, upper);
    return lerp(near, far, level - float(lower));
}
