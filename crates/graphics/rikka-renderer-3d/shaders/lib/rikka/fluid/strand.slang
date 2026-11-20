__exported import rikka.fluid;

static const int RIKKA_FLUID_STRAND_REACH = 16;
static const float2 RIKKA_FLUID_STRAND_WIDTH = float2(8.0, 5.0);

struct RikkaFluidStrand {
    float3 side;
    float across;
    float pixel;
    float thin;
};

RikkaFluidStrand rikka_fluid_empty_strand() {
    RikkaFluidStrand strand;
    strand.side = float3(0.0);
    strand.across = 0.0;
    strand.pixel = 0.0;
    strand.thin = 0.0;
    return strand;
}

int rikka_fluid_strand_edge(RikkaViewRef view, uint smoothed, int2 pixel, int2 way) {
    int2 limit = int2(view.viewport) - 1;
    for (int step = 1; step <= RIKKA_FLUID_STRAND_REACH; ++step) {
        int2 at = pixel + way * step;
        if (any(at < int2(0)) || any(at > limit)) {
            return step;
        }
        if (rikka_fetch(rikka_images[smoothed], at, 0).r <= 0.0) {
            return step;
        }
    }
    return 0;
}

RikkaFluidStrand rikka_fluid_strand(RikkaFrameRef frame, int2 pixel, float2 uv, float3 position,
                                    float distance, float3 eye, float thin) {
    RikkaViewRef view = frame.view;
    RikkaFluidRef fluid = frame.fluid;
    RikkaFluidStrand strand = rikka_fluid_empty_strand();
    if (thin <= 0.0) {
        return strand;
    }
    float2 texel = float(RIKKA_FLUID_THICKNESS_DIVISOR) * view.inverse_viewport;
    float2 gradient = float2(
        rikka_fluid_thin_sample(view, fluid.coverage, uv + float2(texel.x, 0.0)).r
            - rikka_fluid_thin_sample(view, fluid.coverage, uv - float2(texel.x, 0.0)).r,
        rikka_fluid_thin_sample(view, fluid.coverage, uv + float2(0.0, texel.y)).r
            - rikka_fluid_thin_sample(view, fluid.coverage, uv - float2(0.0, texel.y)).r);
    float size = length(gradient);
    if (size < 1e-4) {
        return strand;
    }
    int2 inward = int2(round(gradient / size * 1.4142));
    int inner = rikka_fluid_strand_edge(view, fluid.smoothed, pixel, inward);
    int outer = rikka_fluid_strand_edge(view, fluid.smoothed, pixel, -inward);
    if (inner == 0 || outer == 0) {
        return strand;
    }
    float3 outward = rikka_fluid_position(view, uv - float2(inward) * view.inverse_viewport, distance) - position;
    outward -= eye * dot(outward, eye);
    float reach = length(outward);
    if (!(reach > 1e-12)) {
        return strand;
    }
    float span = float(inner + outer - 1);
    float width = span * length(float2(inward));
    float radius = fluid.radius * rikka_fluid_pixels_per_metre(view, distance);
    strand.side = outward / reach;
    strand.across = float(inner - outer) / span;
    strand.pixel = 2.0 / width;
    strand.thin = thin * smoothstep(RIKKA_FLUID_STRAND_WIDTH.x, RIKKA_FLUID_STRAND_WIDTH.y, width / radius);
    return strand;
}

float3 rikka_fluid_strand_normal(RikkaFluidStrand strand, float3 eye, float across) {
    float lateral = clamp(across, -1.0, 1.0);
    return normalize(eye * sqrt(max(1.0 - lateral * lateral, 1e-4)) + strand.side * lateral);
}

float3 rikka_fluid_surface_normal(RikkaFluidStrand strand, float3 smoothed, float3 eye) {
    if (strand.thin <= 0.0) {
        return smoothed;
    }
    return normalize(lerp(smoothed, rikka_fluid_strand_normal(strand, eye, strand.across), strand.thin));
}
