struct Gradient {
    float4 line;
    float4 offsets[2];
    float4 colors[8];
    uint count;
    uint3 padding;
}

struct Paint {
    Gradient fill;
    Gradient stroke;
}

[[vk::binding(8, 0)]] StructuredBuffer<Paint> node_paints;

float gradient_along(float4 line, float2 p) {
    float2 direction = line.zw - line.xy;
    float length_squared = dot(direction, direction);
    return length_squared > 0.0 ? dot(p - line.xy, direction) / length_squared : 0.0;
}

float gradient_offset(Gradient gradient, uint index) {
    return gradient.offsets[index >> 2u][index & 3u];
}

float4 gradient_at(Gradient gradient, float t) {
    float at = clamp(t, 0.0, 1.0);
    float4 color = gradient.colors[0];
    for (uint i = 1u; i < gradient.count; i++) {
        float from = gradient_offset(gradient, i - 1u);
        float to = gradient_offset(gradient, i);
        if (at > from) {
            float span = to - from;
            float share = span > 0.0 ? clamp((at - from) / span, 0.0, 1.0) : 1.0;
            color = lerp(gradient.colors[i - 1u], gradient.colors[i], share);
        }
    }
    return color;
}
