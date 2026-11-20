__exported import rikka.common;
__exported import rikka.parameters;

static const float RIKKA_EFFECT_REFERENCE_HEIGHT = 1080.0;
static const uint RIKKA_EFFECT_REPEAT = 0u;
static const uint RIKKA_EFFECT_CLAMP = 1u;
static const float RIKKA_EFFECT_MID_GREY = 0.18;
static const uint RIKKA_EFFECT_PLACED_SLOTS = 192u;

[[vk::binding(4, 0)]] StructuredBuffer<uint2> rikka_effect_placed_extents;

uint2 rikka_effect_physical_size(uint image) {
    uint width;
    uint height;
    rikka_images[NonUniformResourceIndex(image)].GetDimensions(width, height);
    return uint2(width, height);
}

uint2 rikka_effect_logical_size(uint image, uint2 physical) {
    if (image >= RIKKA_EFFECT_PLACED_SLOTS) {
        return physical;
    }
    return rikka_effect_placed_extents[image];
}

float2 rikka_effect_placed_uv(uint image, float2 uv) {
    if (image >= RIKKA_EFFECT_PLACED_SLOTS) {
        return uv;
    }
    float2 physical = float2(rikka_effect_physical_size(image));
    return rikka_placed_uv(uv, float2(rikka_effect_placed_extents[image]), physical);
}

bool rikka_effect_outside(uint2 pixel, uint2 size) {
    return any(pixel >= size);
}

float4 rikka_effect_load(uint image, int2 pixel) {
    return rikka_fetch(rikka_images[NonUniformResourceIndex(image)], pixel, 0);
}

float4 rikka_effect_load_clamped(uint image, int2 pixel, uint2 size) {
    return rikka_effect_load(image, clamp(pixel, int2(0), int2(size) - 1));
}

uint rikka_effect_load_layers(uint image, int2 pixel) {
    return rikka_identity_images[NonUniformResourceIndex(image)].Load(int3(pixel, 0)).r;
}

uint rikka_effect_load_layers_clamped(uint image, int2 pixel, uint2 size) {
    return rikka_effect_load_layers(image, clamp(pixel, int2(0), int2(size) - 1));
}

uint2 rikka_effect_layers_size(uint image) {
    uint width;
    uint height;
    rikka_identity_images[NonUniformResourceIndex(image)].GetDimensions(width, height);
    return rikka_effect_logical_size(image, uint2(width, height));
}

float4 rikka_effect_sample(uint image, float2 uv) {
    return rikka_textures[NonUniformResourceIndex(image)].SampleLevel(rikka_effect_placed_uv(image, uv), 0.0);
}

float4 rikka_effect_sample_repeat(uint image, float2 uv) {
    return rikka_images[NonUniformResourceIndex(image)]
        .SampleLevel(rikka_samplers[RIKKA_EFFECT_REPEAT], uv, 0.0);
}

uint2 rikka_effect_image_size(uint image) {
    return rikka_effect_logical_size(image, rikka_effect_physical_size(image));
}

void rikka_effect_store(uint image, int2 pixel, float4 value) {
    rikka_storage_rgba16f[NonUniformResourceIndex(image)][pixel] = value;
}

void rikka_effect_store_r32(uint image, int2 pixel, float value) {
    rikka_storage_r32f[NonUniformResourceIndex(image)][pixel] = value;
}

float2 rikka_effect_uv(int2 pixel, uint2 size) {
    return (float2(pixel) + 0.5) / float2(size);
}

float rikka_effect_scale(uint2 size) {
    return float(size.y) / RIKKA_EFFECT_REFERENCE_HEIGHT;
}

float rikka_effect_aspect(uint2 size) {
    return float(size.x) / float(size.y);
}

float3 rikka_effect_straight(float4 color) {
    return color.a > 1e-5 ? color.rgb / color.a : color.rgb;
}

float3 rikka_effect_encode(float3 linear) {
    return rikka_encode_srgb(max(linear, float3(0.0)));
}

float rikka_effect_linear_depth(RikkaFrameRef frame, float depth) {
    return depth <= 0.0 ? 1e9 : frame.view.near / depth;
}

float2 rikka_effect_rotate(float2 value, float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return float2(c * value.x - s * value.y, s * value.x + c * value.y);
}

uint rikka_effect_hash(uint3 seed) {
    return rikka_pcg_hash(seed.x ^ rikka_pcg_hash(seed.y ^ rikka_pcg_hash(seed.z)));
}

float rikka_effect_random(uint3 seed) {
    return rikka_unit(rikka_effect_hash(seed));
}

float rikka_effect_lattice(int3 cell, uint seed) {
    return rikka_effect_random(uint3(uint(cell.x) ^ (seed * 0x9e3779b9u), uint(cell.y), uint(cell.z)));
}

float rikka_effect_noise(float3 position, uint seed) {
    int3 cell = int3(floor(position));
    float3 f = position - float3(cell);
    float3 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    float x00 = lerp(rikka_effect_lattice(cell, seed), rikka_effect_lattice(cell + int3(1, 0, 0), seed), u.x);
    float x10 = lerp(rikka_effect_lattice(cell + int3(0, 1, 0), seed), rikka_effect_lattice(cell + int3(1, 1, 0), seed), u.x);
    float x01 = lerp(rikka_effect_lattice(cell + int3(0, 0, 1), seed), rikka_effect_lattice(cell + int3(1, 0, 1), seed), u.x);
    float x11 = lerp(rikka_effect_lattice(cell + int3(0, 1, 1), seed), rikka_effect_lattice(cell + int3(1, 1, 1), seed), u.x);
    return lerp(lerp(x00, x10, u.y), lerp(x01, x11, u.y), u.z) * 2.0 - 1.0;
}

float rikka_effect_fbm(float3 position, uint seed, int octaves) {
    float sum = 0.0;
    float weight = 0.5;
    float total = 0.0;
    for (int octave = 0; octave < octaves; ++octave) {
        sum += rikka_effect_noise(position, seed + uint(octave)) * weight;
        total += weight;
        position = position * 2.03 + float3(17.1, 3.7, 5.3);
        weight *= 0.5;
    }
    return sum / total;
}

float rikka_effect_stops(float luma) {
    return log2(max(luma, 1e-8) / RIKKA_EFFECT_MID_GREY);
}

float rikka_effect_bright(float luma, float threshold, float knee) {
    float soft = max(knee, 1e-3);
    return smoothstep(threshold - soft, threshold + soft, rikka_effect_stops(luma));
}

float3 rikka_effect_to_oklab(float3 linear) {
    float l = 0.4122214708 * linear.r + 0.5363325363 * linear.g + 0.0514459929 * linear.b;
    float m = 0.2119034982 * linear.r + 0.6806995451 * linear.g + 0.1073969566 * linear.b;
    float s = 0.0883024619 * linear.r + 0.2817188376 * linear.g + 0.6299787005 * linear.b;
    float3 root = sign(float3(l, m, s)) * pow(abs(float3(l, m, s)), float3(1.0 / 3.0));
    return float3(
        0.2104542553 * root.x + 0.7936177850 * root.y - 0.0040720468 * root.z,
        1.9779984951 * root.x - 2.4285922050 * root.y + 0.4505937099 * root.z,
        0.0259040371 * root.x + 0.7827717662 * root.y - 0.8086757660 * root.z);
}

float3 rikka_effect_from_oklab(float3 lab) {
    float l = lab.x + 0.3963377774 * lab.y + 0.2158037573 * lab.z;
    float m = lab.x - 0.1055613458 * lab.y - 0.0638541728 * lab.z;
    float s = lab.x - 0.0894841775 * lab.y - 1.2914855480 * lab.z;
    l = l * l * l;
    m = m * m * m;
    s = s * s * s;
    return float3(
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s);
}
