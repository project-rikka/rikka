__exported import rikka.common;
import rikka.push;

struct RikkaTexture {
    uint image;
    uint sampler;

    bool bound() {
        return image != RIKKA_NONE;
    }

    float4 sampled(float2 uv) {
        if (image == RIKKA_NONE) {
            return float4(1.0);
        }
        return rikka_images[NonUniformResourceIndex(image)].SampleBias(
            rikka_samplers[NonUniformResourceIndex(sampler)], uv, rikka_push.frame.view.mip_bias);
    }

    float4 sampled_grad(float2 uv, float2 ddx, float2 ddy) {
        if (image == RIKKA_NONE) {
            return float4(1.0);
        }
        return rikka_images[NonUniformResourceIndex(image)].SampleGrad(
            rikka_samplers[NonUniformResourceIndex(sampler)], uv, ddx, ddy);
    }

    float4 sampled_level(float2 uv, float level) {
        if (image == RIKKA_NONE) {
            return float4(1.0);
        }
        return rikka_images[NonUniformResourceIndex(image)].SampleLevel(
            rikka_samplers[NonUniformResourceIndex(sampler)], uv, level);
    }
};
