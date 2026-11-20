import rikka.fluid;

struct Push {
    RikkaFrameRef frame;
    uint source;
    uint target;
    uint mode;
    uint pad;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint MODE_FIRST = 0u;
static const uint MODE_ACROSS = 1u;
static const uint MODE_DOWN = 2u;
static const uint MODE_CLEAN = 3u;

static const float RANGE = 10.0;
static const float CLAMP = 1.0;
static const int REACH = 48;
static const int TAPS = 16;
static const int CLEAN_REACH = 2;
static const float HOLE_MASK = 0.05;
static const float BRIDGE_MASK = 0.05;
static const float BRIDGE = 4.0;

float fetch(int2 pixel) {
    if (any(pixel < int2(0)) || any(pixel >= int2(push.size))) {
        return 0.0;
    }
    if (push.mode != MODE_FIRST) {
        return rikka_fetch(rikka_images[push.source], pixel, 0).r;
    }
    return rikka_fluid_raw_distance(push.frame, push.source, pixel);
}

float3 bridge(int2 pixel, float2 way, int reach, float gap) {
    float2 centre = float2(pixel) + 0.5;
    float ahead = 0.0;
    float behind = 0.0;
    int ahead_offset = 0;
    int behind_offset = 0;
    for (int offset = 1; offset <= reach && (ahead <= 0.0 || behind <= 0.0); ++offset) {
        if (ahead <= 0.0) {
            ahead = fetch(int2(floor(centre + way * float(offset))));
            ahead_offset = offset;
        }
        if (behind <= 0.0) {
            behind = fetch(int2(floor(centre - way * float(offset))));
            behind_offset = offset;
        }
    }
    float seen = max(ahead, behind);
    if (ahead <= 0.0 || behind <= 0.0 || abs(ahead - behind) >= gap) {
        return float3(0.0, 1e30, seen);
    }
    float offsets = float(ahead_offset + behind_offset);
    float found = (ahead * float(behind_offset) + behind * float(ahead_offset)) / offsets;
    return float3(found, offsets * length(way), seen);
}

float filled(int2 pixel, float gap) {
    RikkaFrameRef frame = push.frame;
    RikkaFluidRef fluid = frame.fluid;
    RikkaViewRef view = frame.view;
    float2 uv = (float2(pixel) + 0.5) * view.inverse_viewport;
    float mask = rikka_fluid_thin_sample(view, fluid.coverage, uv).r;
    if (mask < BRIDGE_MASK) {
        return 0.0;
    }
    int enclosing = 0;
    float seen = 0.0;
    float2 nearest = float2(0.0, 1e30);
    for (int way = 0; way < 4; ++way) {
        float3 found = bridge(pixel, float2(RIKKA_FLUID_WAYS[way]), RIKKA_FLUID_FILL_REACH, gap);
        seen = max(seen, found.z);
        if (found.x > 0.0) {
            enclosing += 1;
            nearest = found.y < nearest.y ? found.xy : nearest;
        }
    }
    if (seen <= 0.0) {
        return 0.0;
    }
    if (enclosing == 4 && mask >= HOLE_MASK) {
        return nearest.x;
    }
    float reach = BRIDGE * fluid.radius * rikka_fluid_pixels_per_metre(view, seen);
    if (nearest.y <= reach) {
        return nearest.x;
    }
    int steps = min(int(ceil(reach)), RIKKA_FLUID_FILL_REACH);
    for (int way = 0; way < 4; ++way) {
        float angle = (float(way) + 0.5) * 0.78539816;
        float3 found = bridge(pixel, float2(cos(angle), sin(angle)), steps, gap);
        if (found.x > 0.0 && found.y <= reach) {
            return found.x;
        }
    }
    return 0.0;
}

void narrow(inout float sample_distance, inout float weight, inout float opposite,
            inout float2 range, float centre, float gap, float rounding) {
    if (sample_distance <= 0.0) {
        weight = 0.0;
        opposite = 0.0;
    } else if (sample_distance > range.y) {
        sample_distance = centre + rounding;
    } else if (sample_distance < range.x) {
        weight = 0.0;
        opposite = 0.0;
    } else {
        range = float2(min(range.x, sample_distance - gap), max(range.y, sample_distance + gap));
    }
}

float along(int2 pixel, float centre, int2 direction, float sigma, float gap, float rounding) {
    int reach = min(int(ceil(3.0 * sigma)), REACH);
    int taps = min(reach, TAPS);
    float stride = float(reach) / float(max(taps, 1));
    float two_sigma2 = 2.0 * sigma * sigma;
    float sum = centre;
    float total = 1.0;
    float2 ahead_range = float2(centre - gap, centre + gap);
    float2 behind_range = ahead_range;
    for (int k = 1; k <= taps; ++k) {
        int offset = int(round(float(k) * stride));
        float ahead = fetch(pixel + direction * offset);
        float behind = fetch(pixel - direction * offset);
        float w = exp(-float(offset * offset) / two_sigma2);
        float w_ahead = w;
        float w_behind = w;
        narrow(ahead, w_ahead, w_behind, ahead_range, centre, gap, rounding);
        narrow(behind, w_behind, w_ahead, behind_range, centre, gap, rounding);
        sum += ahead * w_ahead + behind * w_behind;
        total += w_ahead + w_behind;
    }
    return sum / total;
}

float clean(int2 pixel, float centre, float gap, float rounding) {
    float sigma = 0.5 * float(CLEAN_REACH) + 0.25;
    float two_sigma2 = 2.0 * sigma * sigma;
    float sum = centre;
    float total = 1.0;
    float2 range = float2(centre - gap, centre + gap);
    for (int y = 0; y <= CLEAN_REACH; ++y) {
        for (int x = -CLEAN_REACH; x <= CLEAN_REACH; ++x) {
            if (y == 0 && x <= 0) {
                continue;
            }
            int2 offset = int2(x, y);
            float ahead = fetch(pixel + offset);
            float behind = fetch(pixel - offset);
            float w = exp(-float(x * x + y * y) / two_sigma2);
            float w_ahead = w;
            float w_behind = w;
            narrow(ahead, w_ahead, w_behind, range, centre, gap, rounding);
            narrow(behind, w_behind, w_ahead, range, centre, gap, rounding);
            sum += ahead * w_ahead + behind * w_behind;
            total += w_ahead + w_behind;
        }
    }
    return sum / total;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 pixel = int2(global_id.xy);
    if (any(pixel >= int2(push.size))) {
        return;
    }
    RikkaFluidRef fluid = push.frame.fluid;
    float gap = RANGE * fluid.radius;
    float centre = fetch(pixel);
    if (centre <= 0.0 && push.mode == MODE_FIRST) {
        centre = filled(pixel, gap);
    }
    float result = centre;
    if (centre > 0.0) {
        float rounding = CLAMP * fluid.radius;
        RikkaViewRef view = push.frame.view;
        float sigma = fluid.smoothing * rikka_fluid_pixels_per_metre(view, centre);
        if (push.mode == MODE_CLEAN) {
            result = clean(pixel, centre, gap, rounding);
        } else if (sigma >= 0.5) {
            int2 direction = push.mode == MODE_DOWN ? int2(0, 1) : int2(1, 0);
            result = along(pixel, centre, direction, sigma, gap, rounding);
        }
    }
    rikka_storage_r32f[push.target][uint2(pixel)] = result;
}
