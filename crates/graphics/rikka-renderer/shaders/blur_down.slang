import canvas.fx;

[shader("fragment")]
float4 fragment([[vk::location(0)]] float2 v_uv : TEXCOORD0) : SV_Target {
    float2 uv = fx_src_uv(v_uv);
    float2 offset = fx_half_dst_texel() * fx.params[0][0];
    float4 bounds = fx_clamp_bounds();
    float2 lo = bounds.xy;
    float2 hi = bounds.zw;

    float4 sum = fx_sample_level(clamp(uv, lo, hi), 0.0) * 4.0;
    sum += fx_sample_level(clamp(uv - offset, lo, hi), 0.0);
    sum += fx_sample_level(clamp(uv + offset, lo, hi), 0.0);
    sum += fx_sample_level(clamp(uv + float2(offset.x, -offset.y), lo, hi), 0.0);
    sum += fx_sample_level(clamp(uv - float2(offset.x, -offset.y), lo, hi), 0.0);

    return sum * 0.125;
}
