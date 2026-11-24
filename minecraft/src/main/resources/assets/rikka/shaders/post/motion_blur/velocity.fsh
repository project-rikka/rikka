#version 330
#extension GL_ARB_separate_shader_objects : require

uniform sampler2D MainSampler;
uniform sampler2D MainDepthSampler;

layout(std140) uniform VelocityBlur {
    mat4 ViewInverse;
    mat4 ProjectionInverse;
    mat4 PreviousView;
    mat4 PreviousProjection;
    vec3 CameraDelta;
    float Strength;
    int SampleCount;
    int Profile;
};

layout(location = 0) in vec2 texCoord;
layout(location = 0) out vec4 fragColor;

const float PI = 3.141592653589793;
const float FAR_DEPTH = 0.0;
const float NEAR_PASSTHROUGH_DEPTH = 0.44;
const float MAX_VELOCITY = 0.4;
const int PROFILE_WEIGHTED = 1;
const float WEIGHTED_SAMPLE_SCALE = 1.35;
const float WEIGHTED_SPAN = 3.64 * 0.96;

float nearerDepth(float a, float b) {
    return max(a, b);
}

vec2 reproject(vec2 uv, float depth) {
    vec4 view = ProjectionInverse * vec4(uv * 2.0 - 1.0, depth, 1.0);
    vec3 world = (ViewInverse * vec4(view.xyz / view.w, 1.0)).xyz + CameraDelta;
    vec4 previousClip = PreviousProjection * (PreviousView * vec4(world, 1.0));
    return (previousClip.xy / previousClip.w) * 0.5 + 0.5;
}

vec2 clampLength(vec2 velocity) {
    float lengthSquared = dot(velocity, velocity);
    return lengthSquared > MAX_VELOCITY * MAX_VELOCITY ? velocity * (MAX_VELOCITY * inversesqrt(lengthSquared)) : velocity;
}

float interleavedNoise(vec2 position) {
    return fract(52.9829189 * fract(0.06711056 * position.x + 0.00583715 * position.y));
}

float blackmanSinc(float u) {
    float z = 1.89 * u;
    float sinc = abs(z) < 0.0001 ? 1.0 : sin(PI * z) / (PI * z);
    return sinc * (0.42 + 0.5 * cos(PI * u) + 0.08 * cos(2.0 * PI * u));
}

vec2 velocityAt(vec2 uv, float depth) {
    vec2 full = uv - reproject(uv, depth);
#ifdef MOTION_BLUR_FULL
    return full;
#else
    vec2 camera = uv - reproject(uv, FAR_DEPTH);
    float cameraSquared = dot(camera, camera);
    vec2 rotation = cameraSquared > 1e-12 ? camera * (clamp(dot(full, camera), 0.0, cameraSquared) / cameraSquared) : vec2(0.0);
#ifdef MOTION_BLUR_ROTATION
    return rotation;
#else
    return full - rotation;
#endif
#endif
}

void main() {
    ivec2 texel = ivec2(gl_FragCoord.xy);
    float depth = texelFetch(MainDepthSampler, texel, 0).x;
    if (depth > NEAR_PASSTHROUGH_DEPTH) {
        fragColor = texture(MainSampler, texCoord);
        return;
    }

    ivec2 lastTexel = textureSize(MainDepthSampler, 0) - 1;
    float dilatedDepth = depth;
    dilatedDepth = nearerDepth(dilatedDepth, texelFetch(MainDepthSampler, clamp(texel + ivec2(1, 0), ivec2(0), lastTexel), 0).x);
    dilatedDepth = nearerDepth(dilatedDepth, texelFetch(MainDepthSampler, clamp(texel + ivec2(-1, 0), ivec2(0), lastTexel), 0).x);
    dilatedDepth = nearerDepth(dilatedDepth, texelFetch(MainDepthSampler, clamp(texel + ivec2(0, 1), ivec2(0), lastTexel), 0).x);
    dilatedDepth = nearerDepth(dilatedDepth, texelFetch(MainDepthSampler, clamp(texel + ivec2(0, -1), ivec2(0), lastTexel), 0).x);

    vec2 velocity = clampLength(velocityAt(texCoord, dilatedDepth));
    float speed = length(velocity);
    int baseSamples = clamp(int(ceil(speed * float(SampleCount))), 4, SampleCount);
    bool weighted = Profile == PROFILE_WEIGHTED;
    int samples = weighted
        ? clamp(int(ceil(float(baseSamples) * WEIGHTED_SAMPLE_SCALE)), 6, max(6, int(ceil(float(SampleCount) * WEIGHTED_SAMPLE_SCALE))))
        : baseSamples;

    vec2 stride = Strength * velocity * (weighted ? WEIGHTED_SPAN : 1.0) / float(samples);
    float centerOffset = -float(samples) * 0.5;
    vec2 seed = gl_FragCoord.xy;
    vec3 sum = vec3(0.0);
    float totalWeight = 0.0;

    for (int i = 0; i < samples; i++) {
        float index = float(i);
        float offset = index + centerOffset + interleavedNoise(seed + vec2(index, index * 1.4));
        vec3 color = texture(MainSampler, texCoord + offset * stride).rgb;
        if (weighted) {
            float weight = blackmanSinc(offset * 2.0 / float(samples));
            sum += pow(max(color, vec3(0.0)), vec3(2.2)) * weight;
            totalWeight += weight;
        } else {
            sum += color * color;
            totalWeight += 1.0;
        }
    }

    if (totalWeight <= 0.0001) {
        fragColor = texture(MainSampler, texCoord);
    } else if (weighted) {
        fragColor = vec4(pow(clamp(sum / totalWeight, vec3(0.0), vec3(1.0)), vec3(1.0 / 2.2)), 1.0);
    } else {
        fragColor = vec4(sqrt(sum / totalWeight), 1.0);
    }
}
