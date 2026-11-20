struct FxPush {
    float4 src_rect;
    float2 src_texel;
    float2 src_per_dst;
    float4 params[5];
    float2 src_origin;
    float src_scale;
    uint src;
}

[[vk::binding(0, 0)]] Sampler2D fx_textures[];

[[vk::push_constant]] ConstantBuffer<FxPush> fx;

float4 fx_sample_level(float2 uv, float level) {
    return fx_textures[fx.src].SampleLevel(uv, level);
}

float2 fx_src_uv(float2 uv) {
    return lerp(fx.src_rect.xy, fx.src_rect.zw, uv);
}

float2 fx_clamp_src(float2 uv) {
    return clamp(uv, fx.src_rect.xy + 0.5 * fx.src_texel, fx.src_rect.zw - 0.5 * fx.src_texel);
}

float4 fx_clamp_bounds() {
    bool input_clamped = fx.params[1].z > 0.0;
    float2 lo = input_clamped ? fx.params[1].xy : fx.src_rect.xy + 0.5 * fx.src_texel;
    float2 hi = input_clamped ? fx.params[1].zw : fx.src_rect.zw - 0.5 * fx.src_texel;
    return float4(lo, hi);
}

float2 fx_half_dst_texel() {
    return 0.5 * fx.src_per_dst * fx.src_texel;
}
