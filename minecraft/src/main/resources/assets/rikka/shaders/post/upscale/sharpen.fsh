#version 330
#extension GL_ARB_separate_shader_objects : require

uniform sampler2D MainSampler;

layout(std140) uniform Sharpening {
    float Amount;
};

layout(location = 0) in vec2 texCoord;
layout(location = 0) out vec4 fragColor;

const float LOBE_LIMIT = 0.25 - 1.0 / 16.0;
const float MIN_DENOMINATOR = 1.0 / 65536.0;

void main() {
    ivec2 size = textureSize(MainSampler, 0);
    ivec2 limit = size - 1;
    ivec2 pixel = clamp(ivec2(texCoord * vec2(size)), ivec2(0), limit);
    vec4 centre = texelFetch(MainSampler, pixel, 0);
    vec3 up = texelFetch(MainSampler, clamp(pixel + ivec2(0, -1), ivec2(0), limit), 0).rgb;
    vec3 left = texelFetch(MainSampler, clamp(pixel + ivec2(-1, 0), ivec2(0), limit), 0).rgb;
    vec3 right = texelFetch(MainSampler, clamp(pixel + ivec2(1, 0), ivec2(0), limit), 0).rgb;
    vec3 down = texelFetch(MainSampler, clamp(pixel + ivec2(0, 1), ivec2(0), limit), 0).rgb;

    vec3 ringLow = min(min(up, left), min(right, down));
    vec3 ringHigh = max(max(up, left), max(right, down));
    vec3 low = min(ringLow, centre.rgb);
    vec3 high = max(ringHigh, centre.rgb);
    vec3 towardBlack = low / max(4.0 * ringHigh, vec3(MIN_DENOMINATOR));
    vec3 towardWhite = (1.0 - high) / min(4.0 * ringLow - 4.0, vec3(-MIN_DENOMINATOR));
    vec3 lobes = max(-towardBlack, towardWhite);
    float lobe = max(-LOBE_LIMIT, min(max(lobes.r, max(lobes.g, lobes.b)), 0.0)) * Amount;

    vec3 sharpened = (lobe * (up + left + right + down) + centre.rgb) / (4.0 * lobe + 1.0);
    fragColor = vec4(clamp(sharpened, 0.0, 1.0), centre.a);
}
