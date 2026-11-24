#version 330
#extension GL_ARB_separate_shader_objects : require

uniform sampler2D Sample0Sampler;
uniform sampler2D Sample1Sampler;
uniform sampler2D Sample2Sampler;
uniform sampler2D Sample3Sampler;
uniform sampler2D Sample4Sampler;
uniform sampler2D Sample5Sampler;
uniform sampler2D Sample6Sampler;
uniform sampler2D Sample7Sampler;
uniform sampler2D Sample8Sampler;
uniform sampler2D Sample9Sampler;
uniform sampler2D Sample10Sampler;
uniform sampler2D Sample11Sampler;
uniform sampler2D Sample12Sampler;
uniform sampler2D Sample13Sampler;
uniform sampler2D Sample14Sampler;
uniform sampler2D Sample15Sampler;
uniform sampler2D Sample16Sampler;
uniform sampler2D Sample17Sampler;
uniform sampler2D Sample18Sampler;
uniform sampler2D Sample19Sampler;
uniform sampler2D Sample20Sampler;
uniform sampler2D Sample21Sampler;
uniform sampler2D Sample22Sampler;
uniform sampler2D Sample23Sampler;

layout(std140) uniform FrameBlend {
    float InverseTotalWeight;
    int SampleCount;
    vec4 Weights[6];
};

layout(location = 0) in vec2 texCoord;
layout(location = 0) out vec4 fragColor;

const int MAX_SAMPLES = 24;

vec3 sampleColor(int index) {
    switch (index) {
        case 0: return texture(Sample0Sampler, texCoord).rgb;
        case 1: return texture(Sample1Sampler, texCoord).rgb;
        case 2: return texture(Sample2Sampler, texCoord).rgb;
        case 3: return texture(Sample3Sampler, texCoord).rgb;
        case 4: return texture(Sample4Sampler, texCoord).rgb;
        case 5: return texture(Sample5Sampler, texCoord).rgb;
        case 6: return texture(Sample6Sampler, texCoord).rgb;
        case 7: return texture(Sample7Sampler, texCoord).rgb;
        case 8: return texture(Sample8Sampler, texCoord).rgb;
        case 9: return texture(Sample9Sampler, texCoord).rgb;
        case 10: return texture(Sample10Sampler, texCoord).rgb;
        case 11: return texture(Sample11Sampler, texCoord).rgb;
        case 12: return texture(Sample12Sampler, texCoord).rgb;
        case 13: return texture(Sample13Sampler, texCoord).rgb;
        case 14: return texture(Sample14Sampler, texCoord).rgb;
        case 15: return texture(Sample15Sampler, texCoord).rgb;
        case 16: return texture(Sample16Sampler, texCoord).rgb;
        case 17: return texture(Sample17Sampler, texCoord).rgb;
        case 18: return texture(Sample18Sampler, texCoord).rgb;
        case 19: return texture(Sample19Sampler, texCoord).rgb;
        case 20: return texture(Sample20Sampler, texCoord).rgb;
        case 21: return texture(Sample21Sampler, texCoord).rgb;
        case 22: return texture(Sample22Sampler, texCoord).rgb;
        case 23: return texture(Sample23Sampler, texCoord).rgb;
        default: return vec3(0.0);
    }
}

void main() {
    vec3 sum = vec3(0.0);
    for (int i = 0; i < MAX_SAMPLES; i++) {
        if (i >= SampleCount) {
            break;
        }
        vec3 color = sampleColor(i);
        sum += color * color * Weights[i / 4][i % 4];
    }
    fragColor = vec4(sqrt(max(sum * InverseTotalWeight, vec3(0.0))), 1.0);
}
