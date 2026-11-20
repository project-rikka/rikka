__exported import rikka.bounce;

[[vk::binding(3, 0)]] [[vk::image_format("rgba32f")]] RWTexture2D<float4> rikka_storage_rgba32f[];

[[vk::binding(3, 0)]] [[vk::image_format("rgba32ui")]] RWTexture2D<uint4> rikka_storage_rgba32ui[];

static const uint RIKKA_RESERVOIR_AGE = 0xffffu;
static const uint RIKKA_RESERVOIR_SKY = 1u << 16;
static const uint RIKKA_RESERVOIR_CONFIDENCE_SHIFT = 24u;
static const float RIKKA_RESERVOIR_JACOBIAN_LIMIT = 10.0;
static const float RIKKA_RESERVOIR_JACOBIAN_CLAMP = 3.0;

struct RikkaReservoir {
    float3 position;
    float3 normal;
    float3 radiance;
    float weight;
    float count;
    uint age;
    bool sky;
    float confidence;
};

RikkaReservoir rikka_reservoir_empty() {
    RikkaReservoir r;
    r.position = float3(0.0);
    r.normal = float3(0.0, 1.0, 0.0);
    r.radiance = float3(0.0);
    r.weight = 0.0;
    r.count = 0.0;
    r.age = 0u;
    r.sky = false;
    r.confidence = 1.0;
    return r;
}

RikkaReservoir rikka_reservoir_of(RikkaBounceSample sample) {
    RikkaReservoir r = rikka_reservoir_empty();
    r.position = sample.position;
    r.normal = sample.normal;
    r.radiance = rikka_bounce_bounded(sample.radiance);
    r.sky = sample.sky;
    return r;
}

RikkaReservoir rikka_reservoir_load(uint samples, uint states, int2 texel) {
    float4 placed = rikka_fetch(rikka_images[NonUniformResourceIndex(samples)], texel, 0);
    uint4 state = rikka_identity_images[NonUniformResourceIndex(states)].Load(int3(texel, 0));
    RikkaReservoir r;
    r.position = placed.xyz;
    r.weight = placed.w;
    float2 red_green = unpackHalf2x16ToFloat(state.x);
    float2 blue_count = unpackHalf2x16ToFloat(state.y);
    r.radiance = float3(red_green, blue_count.x);
    r.count = blue_count.y;
    r.normal = rikka_decode_octahedral(state.z);
    r.age = state.w & RIKKA_RESERVOIR_AGE;
    r.sky = (state.w & RIKKA_RESERVOIR_SKY) != 0u;
    r.confidence = float(state.w >> RIKKA_RESERVOIR_CONFIDENCE_SHIFT) / 255.0;
    return r;
}

void rikka_reservoir_store(uint samples, uint states, uint2 texel, RikkaReservoir r) {
    uint confidence = uint(round(saturate(r.confidence) * 255.0));
    uint flags = min(r.age, RIKKA_RESERVOIR_AGE) | (r.sky ? RIKKA_RESERVOIR_SKY : 0u)
        | (confidence << RIKKA_RESERVOIR_CONFIDENCE_SHIFT);
    rikka_storage_rgba32f[samples][texel] = float4(r.position, r.weight);
    rikka_storage_rgba32ui[states][texel] = uint4(
        packHalf2x16(r.radiance.rg),
        packHalf2x16(float2(r.radiance.b, r.count)),
        rikka_encode_octahedral(r.normal),
        flags);
}

float3 rikka_reservoir_direction(RikkaReservoir r, float3 from) {
    return r.sky ? r.position : normalize(r.position - from);
}

float rikka_reservoir_target(RikkaReservoir r, float3 position, float3 normal) {
    float facing;
    if (r.sky) {
        facing = dot(normal, r.position);
    } else {
        float3 offset = r.position - position;
        float squared = dot(offset, offset);
        if (squared <= 1e-12) {
            return 0.0;
        }
        float3 direction = offset * rsqrt(squared);
        if (dot(r.normal, direction) >= 0.0) {
            return 0.0;
        }
        facing = dot(normal, direction);
    }
    return rikka_luma(r.radiance) * max(facing, 0.0);
}

float rikka_reservoir_jacobian(RikkaReservoir r, float3 from, float3 to) {
    if (r.sky) {
        return 1.0;
    }
    float3 original = from - r.position;
    float3 shifted = to - r.position;
    float original_squared = dot(original, original);
    float shifted_squared = dot(shifted, shifted);
    float original_cosine = dot(r.normal, original) * rsqrt(original_squared);
    float shifted_cosine = dot(r.normal, shifted) * rsqrt(shifted_squared);
    float jacobian = (shifted_cosine * original_squared) / (original_cosine * shifted_squared);
    if (!(jacobian > 1.0 / RIKKA_RESERVOIR_JACOBIAN_LIMIT && jacobian < RIKKA_RESERVOIR_JACOBIAN_LIMIT)) {
        return 0.0;
    }
    return clamp(jacobian, 1.0 / RIKKA_RESERVOIR_JACOBIAN_CLAMP, RIKKA_RESERVOIR_JACOBIAN_CLAMP);
}

float3 rikka_reservoir_irradiance(RikkaReservoir r, float3 position, float3 normal) {
    float facing = max(dot(normal, rikka_reservoir_direction(r, position)), 0.0);
    float3 irradiance = r.radiance * (facing * r.weight / RIKKA_PI);
    return any(isnan(irradiance)) || any(isinf(irradiance)) ? float3(0.0) : irradiance;
}

float2 rikka_reservoir_random(uint2 pixel, uint frame, uint stream) {
    uint seed = rikka_pcg_hash(pixel.x + rikka_pcg_hash(pixel.y + rikka_pcg_hash(frame + stream * 0x9e3779b9u)));
    return float2(rikka_unit(seed), rikka_unit(rikka_hash(seed)));
}

struct RikkaResampling {
    RikkaReservoir chosen;
    float target;
    float sum;
    float count;
    uint index;
};

RikkaResampling rikka_resampling_empty() {
    RikkaResampling s;
    s.chosen = rikka_reservoir_empty();
    s.target = 0.0;
    s.sum = 0.0;
    s.count = 0.0;
    s.index = RIKKA_NONE;
    return s;
}

void rikka_resampling_add(inout RikkaResampling s, RikkaReservoir candidate, float target, float weight,
    float count, float random, uint index) {
    s.count += count;
    if (!(weight > 0.0) || isinf(weight)) {
        return;
    }
    s.sum += weight;
    if (random * s.sum < weight) {
        s.chosen = candidate;
        s.target = target;
        s.index = index;
    }
}

float rikka_resampling_weight(RikkaResampling s, float normalization) {
    float denominator = normalization * s.target;
    return denominator > 0.0 ? s.sum / denominator : 0.0;
}
