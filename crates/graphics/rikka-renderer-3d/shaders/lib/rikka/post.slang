__exported import rikka.common;

static const uint POST_LINEAR_CLAMP = 1u;
static const float POST_SKY_DEPTH = 65000.0;

float4 post_sample(uint image, float2 uv) {
    return rikka_images[image].SampleLevel(rikka_samplers[POST_LINEAR_CLAMP], uv, 0.0);
}

float4 post_fetch(uint image, int2 texel) {
    return rikka_fetch(rikka_images[image], texel, 0);
}

float4 post_fetch_clamped(uint image, int2 texel, int2 size) {
    return rikka_fetch(rikka_images[image], clamp(texel, int2(0), size - 1), 0);
}

void post_store(uint image, int2 texel, float4 value) {
    rikka_storage_rgba16f[image][uint2(texel)] = value;
}

void post_store_scalar(uint image, int2 texel, float value) {
    rikka_storage_r32f[image][uint2(texel)] = value;
}

float post_exposure(uint image) {
    return rikka_fetch(rikka_images[image], int2(2, 0), 0).r;
}

float post_exposure_or(uint image, float fallback) {
    return image == RIKKA_NONE ? fallback : post_exposure(image);
}

float post_random(uint3 seed) {
    uint mixed = rikka_pcg_hash(seed.x ^ rikka_pcg_hash(seed.y ^ rikka_pcg_hash(seed.z)));
    return float(mixed) * (1.0 / 4294967296.0);
}

float post_polygon(float angle, float blades, float rotation) {
    if (blades < 3.0) {
        return 1.0;
    }
    float sector = RIKKA_TAU / blades;
    float local = rikka_mod(angle - rotation, sector) - 0.5 * sector;
    float edge = cos(0.5 * sector) / cos(local);
    return lerp(edge, 1.0, 0.12);
}
