import plane;

struct Push {
    Head head;
    float scale;
    float support;
    uint filter_kind;
    int start;
    int offset;
    int limit;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint NEAREST = 0u;
static const uint BOX = 1u;
static const uint TRIANGLE = 2u;
static const uint CATMULL_ROM = 3u;
static const uint MITCHELL = 4u;
static const uint LANCZOS3 = 5u;
static const uint B_SPLINE = 6u;

static const float THIRD = float(1.0l / 3.0l);

float bicubic(float x, float b, float c) {
    x = abs(x);
    if (x < 1.0) {
        return ((12.0 - 9.0 * b - 6.0 * c) * x * x * x + (-18.0 + 12.0 * b + 6.0 * c) * x * x + (6.0 - 2.0 * b)) / 6.0;
    }
    if (x < 2.0) {
        return ((-b - 6.0 * c) * x * x * x + (6.0 * b + 30.0 * c) * x * x + (-12.0 * b - 48.0 * c) * x + (8.0 * b + 24.0 * c)) / 6.0;
    }
    return 0.0;
}

float sinc(float x) {
    if (abs(x) < 1e-6) {
        return 1.0;
    }
    float a = 3.14159265358979 * x;
    return sin(a) / a;
}

float weight(float x) {
    switch (pc.filter_kind) {
    case BOX: return (x >= -0.5 && x < 0.5) ? 1.0 : 0.0;
    case TRIANGLE: return max(1.0 - abs(x), 0.0);
    case CATMULL_ROM: return bicubic(x, 0.0, 0.5);
    case MITCHELL: return bicubic(x, THIRD, THIRD);
    case LANCZOS3: return abs(x) < 3.0 ? sinc(x) * sinc(x / 3.0) : 0.0;
    case B_SPLINE: return bicubic(x, 1.0, 0.0);
    default: return 0.0;
    }
}

int source(int at) {
    return clamp(at, 0, pc.limit - 1) - pc.offset;
}

[shader("compute")]
[numthreads(16, 16, 1)]
void resample(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    bool across = pc.head.mode == 0u;
    int2 step = across ? int2(1, 0) : int2(0, 1);
    float position = float((across ? pixel.x : pixel.y) + pc.start);
    float center = (position + 0.5) / pc.scale;
    int2 base = across ? int2(0, pixel.y) : int2(pixel.x, 0);
    if (pc.filter_kind == NEAREST) {
        int nearest = int(floor(center));
        pc.head.dst.store(pixel, pc.head.src.load(base + step * source(nearest)));
        return;
    }
    float stretch = min(pc.scale, 1.0);
    float reach = pc.support / stretch;
    int first = int(floor(center - reach));
    int last = int(ceil(center + reach));
    float4 sum = float4(0.0);
    float total = 0.0;
    for (int i = first; i <= last; i++) {
        float w = weight((float(i) + 0.5 - center) * stretch);
        if (w == 0.0) {
            continue;
        }
        sum += w * pc.head.src.load(base + step * source(i));
        total += w;
    }
    float4 value = total != 0.0 ? sum / total : pc.head.src.load(base + step * source(int(floor(center))));
    pc.head.dst.store(pixel, pc.head.clamped(value));
}
