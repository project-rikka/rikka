#version 330
#extension GL_ARB_separate_shader_objects : require

uniform sampler2D MainSampler;
uniform sampler2D HistorySampler;

layout(std140) uniform Accumulation {
    float BlendFactor;
};

layout(location = 0) in vec2 texCoord;
layout(location = 0) out vec4 fragColor;

void main() {
    vec3 current = texture(MainSampler, texCoord).rgb;
    vec3 history = texture(HistorySampler, texCoord).rgb;
#ifdef MOTION_BLUR_ACCUMULATE_MAX
    fragColor = vec4(max(current, history * BlendFactor), 1.0);
#else
    fragColor = vec4(sqrt(mix(current * current, history * history, BlendFactor)), 1.0);
#endif
}
