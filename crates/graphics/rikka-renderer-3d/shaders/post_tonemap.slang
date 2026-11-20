import rikka.post;
import rikka.post.tone;

struct Push {
    uint source;
    uint tone;
    uint look;
    float preserve_hue;
    float peak;
    float contrast;
    float toe;
    float linear_length;
    float toe_power;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    PostTone tone;
    tone.tone = push.tone;
    tone.look = push.look;
    tone.preserve_hue = push.preserve_hue;
    tone.curve.peak = push.peak;
    tone.curve.contrast = push.contrast;
    tone.curve.toe = push.toe;
    tone.curve.linear_length = push.linear_length;
    tone.curve.toe_power = push.toe_power;
    float4 color = post_fetch(push.source, int2(frag_coord.xy));
    float alpha = clamp(color.a, 0.0, 1.0);
    float3 straight = alpha > 1e-4 ? color.rgb / alpha : float3(0.0);
    float3 display = clamp(post_tone_map(tone, straight), 0.0, 1.0);
    return float4(rikka_encode_srgb(display) * alpha, alpha);
}
