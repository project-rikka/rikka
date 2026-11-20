import rikka.fluid;

struct Push {
    RikkaFrameRef frame;
    uint thickness;
    uint scattering;
    uint coverage;
    uint seeding;
    uint thickness_target;
    uint scattering_target;
    uint coverage_target;
    uint pad;
    int2 direction;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int REACH = 32;
static const int TAPS = 12;

float distance_at(RikkaFrameRef frame, int2 pixel) {
    int2 full = clamp(pixel * 2, int2(0), int2(frame.view.viewport) - 1);
    return rikka_fluid_raw_distance(frame, frame.fluid.depth, full);
}

float reference(RikkaFrameRef frame, int2 pixel) {
    float distance = distance_at(frame, pixel);
    if (distance > 0.0) {
        return distance;
    }
    for (int k = 1; k <= TAPS; ++k) {
        int offset = (k * REACH) / TAPS;
        float ahead = distance_at(frame, pixel + push.direction * offset);
        if (ahead > 0.0) {
            return ahead;
        }
        float behind = distance_at(frame, pixel - push.direction * offset);
        if (behind > 0.0) {
            return behind;
        }
    }
    return 0.0;
}

float covered(int2 pixel, float4 thickness) {
    if (push.seeding != 0u) {
        return step(1e-6, thickness.a);
    }
    return rikka_fetch(rikka_images[push.coverage], pixel, 0).r;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 pixel = int2(global_id.xy);
    if (any(pixel >= int2(push.size))) {
        return;
    }
    float4 thickness = rikka_fetch(rikka_images[push.thickness], pixel, 0);
    float4 scattering = rikka_fetch(rikka_images[push.scattering], pixel, 0);
    float coverage = covered(pixel, thickness);
    RikkaFrameRef frame = push.frame;
    RikkaViewRef view = frame.view;
    RikkaFluidRef fluid = frame.fluid;
    float distance = reference(frame, pixel);
    float sigma = distance > 0.0 ? 0.5 * rikka_fluid_spread(view, fluid, distance) : 0.0;
    int reach = min(int(ceil(2.5 * sigma)), REACH);
    if (sigma >= 0.5 && reach > 0) {
        int taps = min(reach, TAPS);
        float stride = float(reach) / float(taps);
        float two_sigma2 = 2.0 * sigma * sigma;
        int2 limit = int2(push.size) - 1;
        float total = 1.0;
        for (int k = 1; k <= taps; ++k) {
            int offset = int(round(float(k) * stride));
            float w = exp(-float(offset * offset) / two_sigma2);
            int2 ahead = clamp(pixel + push.direction * offset, int2(0), limit);
            int2 behind = clamp(pixel - push.direction * offset, int2(0), limit);
            float4 thick_ahead = rikka_fetch(rikka_images[push.thickness], ahead, 0);
            float4 thick_behind = rikka_fetch(rikka_images[push.thickness], behind, 0);
            thickness += w * (thick_ahead + thick_behind);
            scattering += w * (rikka_fetch(rikka_images[push.scattering], ahead, 0)
                + rikka_fetch(rikka_images[push.scattering], behind, 0));
            coverage += w * (covered(ahead, thick_ahead) + covered(behind, thick_behind));
            total += 2.0 * w;
        }
        thickness /= total;
        scattering /= total;
        coverage /= total;
    }
    rikka_storage_rgba16f[push.thickness_target][uint2(pixel)] = thickness;
    rikka_storage_rgba16f[push.scattering_target][uint2(pixel)] = scattering;
    rikka_storage_r32f[push.coverage_target][uint2(pixel)] = coverage;
}
