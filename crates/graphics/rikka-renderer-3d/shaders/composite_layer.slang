import rikka.post;

static const uint LAYER_EMISSION = 0u;
static const uint LAYER_OCCLUSION = 1u;
static const uint LAYER_MASK = 2u;
static const uint LAYER_COPY = 3u;

struct Push {
    uint source;
    uint kind;
    uint exposure;
    float exposure_value;
    float2 size;
    float2 source_size;
    float2 source_capacity;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    int2 pixel = int2(frag_coord.xy);
    if (push.kind == LAYER_COPY) {
        return post_fetch(push.source, pixel);
    }
    if (push.kind == LAYER_MASK) {
        uint layers = rikka_identity_images[NonUniformResourceIndex(push.source)].Load(int3(pixel, 0)).r;
        return float4(uint4(layers, layers >> 1u, layers >> 2u, layers >> 3u) & 1u);
    }
    if (push.source == RIKKA_NONE) {
        return float4(1.0);
    }
    float2 uv = rikka_placed_uv((float2(pixel) + 0.5) / push.size, push.source_size, push.source_capacity);
    float4 sampled = post_sample(push.source, uv);
    if (push.kind == LAYER_OCCLUSION) {
        return float4(sampled.rrr, 1.0);
    }
    return float4(max(sampled.rgb, float3(0.0)) * post_exposure_or(push.exposure, push.exposure_value), max(sampled.a, 0.0));
}
