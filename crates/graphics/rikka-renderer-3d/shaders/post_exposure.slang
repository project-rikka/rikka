import rikka.post;
import rikka.surface;

struct Push {
    uint source;
    uint state;
    uint snap;
    float time;
    float adaptation;
    float compensation;
    float reference;
    float night;
    float darkest;
    float brightest;
    float rise;
    float fall;
    uint surface;
    float figure_weight;
    uint2 surface_size;
    float low_percentile;
    float high_percentile;
    float center_weight;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint BINS = 64u;
static const uint GRID = 128u;
static const float LOW_EV = -18.0;
static const float HIGH_EV = 8.0;
static const float LONGEST_STEP = 0.5;

groupshared uint histogram[BINS];

float bin_ev(uint bin) {
    return LOW_EV + (float(bin) + 0.5) * (HIGH_EV - LOW_EV) / float(BINS);
}

float metering(float2 uv) {
    if (push.surface == RIKKA_NONE || push.figure_weight >= 1.0) {
        return 1.0;
    }
    uint2 size = push.surface_size;
    int2 pixel = int2(min(uint2(uv * float2(size)), max(size, uint2(1u)) - 1u));
    return rikka_read_surface(push.surface, pixel).figure != 0u ? push.figure_weight : 1.0;
}

float metered_ev() {
    float total = 0.0;
    for (uint bin = 0u; bin < BINS; ++bin) {
        total += float(histogram[bin]);
    }
    if (total <= 0.0) {
        return push.reference;
    }
    float low = total * push.low_percentile;
    float high = total * push.high_percentile;
    float seen = 0.0;
    float sum = 0.0;
    float kept = 0.0;
    for (uint bin = 0u; bin < BINS; ++bin) {
        float count = float(histogram[bin]);
        float inside = clamp(seen + count, low, high) - clamp(seen, low, high);
        sum += inside * bin_ev(bin);
        kept += inside;
        seen += count;
    }
    return kept > 0.0 ? sum / kept : push.reference;
}

[shader("compute")]
[numthreads(256, 1, 1)]
void compute_main(uint local_index: SV_GroupIndex) {
    uint index = local_index;
    if (index < BINS) {
        histogram[index] = 0u;
    }
    GroupMemoryBarrierWithGroupSync();

    for (uint cell = index; cell < GRID * GRID; cell += 256u) {
        float2 uv = (float2(float(cell % GRID), float(cell / GRID)) + 0.5) / float(GRID);
        float4 color = post_sample(push.source, uv);
        if (color.a <= 1e-3) {
            continue;
        }
        float luminance = rikka_luma(max(color.rgb, float3(0.0))) / color.a;
        float2 centered = uv * 2.0 - 1.0;
        float center = push.center_weight;
        float weight = color.a * ((1.0 - center) + center * exp(-2.0 * dot(centered, centered))) * metering(uv);
        float ev = log2(max(luminance, 1e-9));
        float position = (ev - LOW_EV) / (HIGH_EV - LOW_EV) * float(BINS);
        uint bin = uint(clamp(position, 0.0, float(BINS - 1u)));
        InterlockedAdd(histogram[bin], uint(weight * 64.0 + 0.5));
    }
    GroupMemoryBarrierWithGroupSync();

    if (index != 0u) {
        return;
    }
    float metered = metered_ev();
    float pivot = push.reference - 1.0;
    float felt = metered < pivot ? pivot + (metered - pivot) * push.night : metered;
    float target = clamp(push.reference - felt, push.darkest, push.brightest) * push.adaptation;

    float previous = rikka_storage_r32f[push.state][uint2(0u, 0u)];
    float last = rikka_storage_r32f[push.state][uint2(1u, 0u)];
    float elapsed = push.time - last;
    bool settled = push.snap == 0u && elapsed > 0.0 && elapsed <= LONGEST_STEP
        && !isnan(previous) && !isinf(previous);
    float adapted = target;
    if (settled) {
        float rate = target > previous ? push.rise : push.fall;
        adapted = previous + (target - previous) * (1.0 - exp(-elapsed * rate));
    }
    post_store_scalar(push.state, int2(0, 0), adapted);
    post_store_scalar(push.state, int2(1, 0), push.time);
    post_store_scalar(push.state, int2(2, 0), exp2(adapted + push.compensation));
}
