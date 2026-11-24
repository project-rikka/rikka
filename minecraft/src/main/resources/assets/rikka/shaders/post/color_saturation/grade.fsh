#version 330
#extension GL_ARB_separate_shader_objects : require

uniform sampler2D MainSampler;

layout(std140) uniform ColorGrade {
    vec4 RedRow;
    vec4 GreenRow;
    vec4 BlueRow;
};

layout(location = 0) in vec2 texCoord;
layout(location = 0) out vec4 fragColor;

void main() {
    vec4 source = texture(MainSampler, texCoord);
    vec3 graded = vec3(
        dot(RedRow.rgb, source.rgb) + RedRow.a,
        dot(GreenRow.rgb, source.rgb) + GreenRow.a,
        dot(BlueRow.rgb, source.rgb) + BlueRow.a
    );
    fragColor = vec4(clamp(graded, 0.0, 1.0), source.a);
}
