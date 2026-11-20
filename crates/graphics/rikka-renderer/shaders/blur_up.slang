import canvas.fx;

[shader("fragment")]
float4 fragment([[vk::location(0)]] float2 v_uv : TEXCOORD0) : SV_Target {
    float2 uv = fx_src_uv(v_uv);
    float2 h = fx_half_dst_texel() * fx.params[0][0];
    float4 bounds = fx_clamp_bounds();
    float2 lo = bounds.xy;
    float2 hi = bounds.zw;

    float4 sum = fx_sample_level(clamp(uv + float2(-h.x * 2.0, 0.0), lo, hi), 0.0);
    sum += fx_sample_level(clamp(uv + float2(-h.x, h.y), lo, hi), 0.0) * 2.0;
    sum += fx_sample_level(clamp(uv + float2(0.0, h.y * 2.0), lo, hi), 0.0);
    sum += fx_sample_level(clamp(uv + float2(h.x, h.y), lo, hi), 0.0) * 2.0;
    sum += fx_sample_level(clamp(uv + float2(h.x * 2.0, 0.0), lo, hi), 0.0);
    sum += fx_sample_level(clamp(uv + float2(h.x, -h.y), lo, hi), 0.0) * 2.0;
    sum += fx_sample_level(clamp(uv + float2(0.0, -h.y * 2.0), lo, hi), 0.0);
    sum += fx_sample_level(clamp(uv + float2(-h.x, -h.y), lo, hi), 0.0) * 2.0;

    return sum * (1.0 / 12.0);
}
