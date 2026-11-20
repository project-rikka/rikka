import rikka.surface;
import rikka.temporal;

struct Push {
    RikkaFrameRef frame;
    uint color;
    uint depth;
    uint surface;
    uint shaded;
    uint opaque;
    uint history;
    uint history_meta;
    uint previous_depth;
    uint output;
    uint output_meta;
    uint post_depth;
    uint post_motion;
    uint2 render;
    uint2 display;
    float2 sample;
    float limit;
    uint flags;
    uint metered;
    float exposure;
    float knee;
    uint blended;
    uint emitter_motion;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint RESET = 1u;
static const uint SETTLED = 2u;
static const uint UPSCALED = 4u;
static const uint STILL = 8u;

static const uint LINEAR_CLAMP = 1u;
static const uint NEAREST_CLAMP = 3u;

static const float KERNEL_SHARPNESS = 2.29;
static const float MOVING_GAMMA = 1.0;
static const float LOCK_GAMMA = 2.0;
static const float FLICKER_GAMMA = 1.0;
static const float EDGE_GAMMA = 0.75;
static const float EDGE_LIMIT = 3.0;
static const float CHANGED_LIMIT = 2.0;
static const float CHANGE_FLOOR = 0.05;
static const float MOVING_PIXELS = 0.05;
static const float REACTIVE_SCALE = 4.0;
static const float REACTIVE_MAX = 0.9;
static const float REACTIVE_LOCK = 0.2;
static const float DISOCCLUSION = 0.02;
static const float EDGE_DEPTH = 1.001;
static const float LOCK_CONTRAST = 0.04;
static const float LOCK_DECAY = 1.0 / 16.0;
static const float LOCK_DRIFT = 0.25;
static const float WEIGHT_FLOOR = 1e-4;
static const float CONFIDENT = 4.0;
static const float BOX_INNER = 0.35;
static const float BOX_OUTER = 0.65;
static const float EMITTER_FAINT = 1e-4;
static const float EMITTER_OWNED = 0.5;

float max3(float3 v) {
    return max(v.x, max(v.y, v.z));
}

float4 perceptual(float4 color, float exposure) {
    float3 rgb = max(color.rgb, 0.0) * exposure;
    return float4(rgb / (1.0 + push.knee * max3(rgb)), saturate(color.a));
}

float4 linear_of(float4 color, float exposure) {
    float3 rgb = color.rgb / max(1.0 - push.knee * max3(color.rgb), 1e-4);
    return float4(rgb / exposure, color.a);
}

float4 ycocg(float4 c) {
    return float4(
        dot(c.rgb, float3(0.25, 0.5, 0.25)),
        dot(c.rgb, float3(0.5, 0.0, -0.5)),
        dot(c.rgb, float3(-0.25, 0.5, -0.25)),
        c.a);
}

float4 rgb_of(float4 c) {
    return float4(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z, c.w);
}

float4 load_color(uint image, int2 pixel) {
    return rikka_fetch(rikka_images[NonUniformResourceIndex(image)], pixel, 0);
}

float load_depth(uint image, int2 pixel) {
    return rikka_fetch(rikka_images[NonUniformResourceIndex(image)], pixel, 0).r;
}

float4 sample_linear(uint image, float2 uv) {
    return rikka_images[NonUniformResourceIndex(image)].SampleLevel(rikka_samplers[LINEAR_CLAMP], uv, 0.0);
}

float4 catmull_rom(uint image, float2 uv, float2 size) {
    float2 position = uv * size;
    float2 centre = floor(position - 0.5) + 0.5;
    float2 f = position - centre;
    float2 w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    float2 w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    float2 w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    float2 w3 = f * f * (-0.5 + 0.5 * f);
    float2 w12 = w1 + w2;
    float2 texel = 1.0 / size;
    float2 at0 = (centre - 1.0) * texel;
    float2 at3 = (centre + 2.0) * texel;
    float2 at12 = (centre + w2 / w12) * texel;
    float4 result = sample_linear(image, float2(at12.x, at0.y)) * (w12.x * w0.y)
        + sample_linear(image, float2(at0.x, at12.y)) * (w0.x * w12.y)
        + sample_linear(image, at12) * (w12.x * w12.y)
        + sample_linear(image, float2(at3.x, at12.y)) * (w3.x * w12.y)
        + sample_linear(image, float2(at12.x, at3.y)) * (w12.x * w3.y);
    float total = w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y;
    return result / total;
}

float4 clip_toward(float4 low, float4 high, float4 history) {
    float4 centre = 0.5 * (high + low);
    float4 extent = 0.5 * (high - low) + 1e-5;
    float4 offset = history - centre;
    float4 units = abs(offset / extent);
    float reach = max(max(units.x, units.y), max(units.z, units.w));
    return reach > 1.0 ? centre + offset / reach : history;
}

struct Closest {
    int2 pixel;
    float depth;
};

float4 gather_depth(uint image, int2 corner, float2 capacity) {
    return rikka_images[NonUniformResourceIndex(image)].GatherRed(
        rikka_samplers[NEAREST_CLAMP], (float2(corner) + 1.0) / capacity);
}

float4 clamped_depths(uint image, int2 corner, int2 last) {
    return float4(
        load_depth(image, clamp(corner + int2(0, 1), int2(0), last)),
        load_depth(image, clamp(corner + int2(1, 1), int2(0), last)),
        load_depth(image, clamp(corner + int2(1, 0), int2(0), last)),
        load_depth(image, clamp(corner, int2(0), last)));
}

Closest closest_depth<let REACH : int>(int2 centre, int2 last, float2 capacity) {
    static const int SIDE = 2 * (REACH + 1);
    float depths[SIDE * SIDE];
    int2 origin = centre - REACH;
    bool inside = all(centre + REACH <= last);
    [ForceUnroll]
    for (int by = 0; by <= REACH; ++by) {
        [ForceUnroll]
        for (int bx = 0; bx <= REACH; ++bx) {
            int2 corner = origin + 2 * int2(bx, by);
            float4 gathered;
            if (inside) {
                gathered = gather_depth(push.depth, corner, capacity);
            } else {
                gathered = clamped_depths(push.depth, corner, last);
            }
            int at = 2 * by * SIDE + 2 * bx;
            depths[at] = gathered.w;
            depths[at + 1] = gathered.z;
            depths[at + SIDE] = gathered.x;
            depths[at + SIDE + 1] = gathered.y;
        }
    }
    Closest best;
    best.pixel = centre;
    best.depth = -1.0;
    [ForceUnroll]
    for (int y = -REACH; y <= REACH; ++y) {
        [ForceUnroll]
        for (int x = -REACH; x <= REACH; ++x) {
            float depth = depths[(y + REACH) * SIDE + x + REACH];
            if (depth > best.depth) {
                best.depth = depth;
                best.pixel = clamp(centre + int2(x, y), int2(0), last);
            }
        }
    }
    return best;
}

bool disoccluded(RikkaViewRef view, RikkaScene scene, float2 uv, float2 previous_uv, float depth) {
    if (push.previous_depth == RIKKA_NONE) {
        return false;
    }
    float expected = 0.0;
    if (depth > 0.0) {
        float4 point = rikka_unjittered_point(view, uv, depth);
        float4 before = mul(scene.previous_view_projection, float4(point.xyz / point.w, 1.0));
        expected = before.w > 1e-7 ? before.z / before.w : 0.0;
    }
    float2 previous_size = view.previous_viewport;
    int2 last = int2(previous_size) - 1;
    int2 base = int2(floor(previous_uv * previous_size - 0.5));
    float4 around;
    if (all(base < last)) {
        around = gather_depth(push.previous_depth, base, view.capacity);
    } else {
        around = clamped_depths(push.previous_depth, base, last);
    }
    float stored = min(min(around.x, around.y), min(around.z, around.w));
    return stored > expected * (1.0 + DISOCCLUSION) + 1e-6;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (any(uint2(pixel) >= push.display)) {
        return;
    }
    RikkaViewRef view = push.frame.view;
    RikkaScene scene = push.frame.look.scene;
    float2 display_size = float2(push.display);
    float2 render_size = float2(push.render);
    float2 ratio = render_size / display_size;
    int2 last = int2(push.render) - 1;
    bool upscaled = (push.flags & UPSCALED) != 0u;

    float2 uv = (float2(pixel) + 0.5) / display_size;
    float2 at = uv * render_size;
    int2 centre = clamp(int2(floor(at)), int2(0), last);

    float shaded = load_depth(push.shaded, centre);
    float gbuffer = load_depth(push.depth, centre);
    bool edge = shaded > gbuffer * EDGE_DEPTH + 1e-7;

    Closest closest = edge ? closest_depth<2>(centre, last, view.capacity)
        : closest_depth<1>(centre, last, view.capacity);
    float2 velocity = closest.depth > 0.0
        ? rikka_read_surface(push.surface, closest.pixel).velocity
        : rikka_sky_motion(view, scene.previous_sky_view_projection, uv);
    float emitted = 0.0;
    if (push.emitter_motion != RIKKA_NONE) {
        float4 moving = load_color(push.emitter_motion, centre);
        if (moving.z > EMITTER_FAINT) {
            emitted = saturate(moving.z);
            velocity = lerp(velocity, moving.xy / moving.z, emitted);
        }
    }
    if (push.post_depth != RIKKA_NONE) {
        rikka_storage_r32f[NonUniformResourceIndex(push.post_depth)][pixel] = max(closest.depth, 0.0);
        rikka_storage_rgba16f[NonUniformResourceIndex(push.post_motion)][pixel] =
            float4(velocity, 0.0, closest.depth > 0.0 ? 1.0 : 0.0);
    }

    float2 previous_uv = uv - velocity;
    float4 previous_meta = rikka_fetch(rikka_images[NonUniformResourceIndex(push.history_meta)],
        clamp(int2(previous_uv * display_size), int2(0), int2(push.display) - 1), 0);

    if ((push.flags & SETTLED) != 0u && (push.flags & RESET) == 0u) {
        float4 kept = load_color(push.history, pixel);
        float4 kept_meta = load_color(push.history_meta, pixel);
        rikka_storage_rgba16f[NonUniformResourceIndex(push.output)][pixel] = kept;
        rikka_storage_rgba16f[NonUniformResourceIndex(push.output_meta)][pixel] = kept_meta;
        return;
    }

    float exposure = push.metered != RIKKA_NONE
        ? rikka_fetch(rikka_images[NonUniformResourceIndex(push.metered)], int2(2, 0), 0).r
        : push.exposure;
    exposure = max(exposure, 1e-6);
    bool still = (push.flags & STILL) != 0u;
    bool valid = (push.flags & RESET) == 0u
        && all(previous_uv >= 0.0) && all(previous_uv <= 1.0)
        && (still || emitted > EMITTER_OWNED || !disoccluded(view, scene, uv, previous_uv, closest.depth));
    float accumulated = valid ? previous_meta.x : 0.0;
    float boxed = still ? saturate(accumulated / CONFIDENT) : 0.0;

    float4 sum = float4(0.0);
    float weights = 0.0;
    float4 first = float4(0.0);
    float4 second = float4(0.0);
    float4 low = float4(1e9);
    float4 high = float4(-1e9);
    float lumas[9];
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            int2 tap = centre + int2(x, y);
            float4 color = perceptual(load_color(push.color, clamp(tap, int2(0), last)), exposure);
            float4 shape = ycocg(color);
            first += shape;
            second += shape * shape;
            low = min(low, shape);
            high = max(high, shape);
            lumas[(y + 1) * 3 + x + 1] = shape.x;
            float2 offset = float2(tap) + 0.5 + push.sample - at;
            float wide = exp(-KERNEL_SHARPNESS * dot(offset, offset));
            float2 inside = 1.0 - smoothstep(BOX_INNER, BOX_OUTER, abs(offset / ratio));
            float weight = lerp(wide, inside.x * inside.y, boxed);
            sum += color * weight;
            weights += weight;
        }
    }
    float4 current;
    float frame_weight;
    if (upscaled) {
        current = sum / max(weights, WEIGHT_FLOOR);
        frame_weight = weights;
    } else {
        current = perceptual(load_color(push.color, centre), exposure);
        frame_weight = 1.0;
    }
    float4 current_shape = ycocg(current);

    float reactive = 0.0;
    if (push.opaque != RIKKA_NONE) {
        float4 behind = perceptual(load_color(push.opaque, centre), exposure);
        float4 front = perceptual(load_color(push.blended, centre), exposure);
        float difference = max3(abs(front.rgb - behind.rgb)) + abs(front.a - behind.a);
        reactive = saturate(difference * REACTIVE_SCALE) * REACTIVE_MAX * (1.0 - emitted);
    }

    float centre_luma = lumas[4];
    float ridge_up = min(min(lumas[1], lumas[7]), min(lumas[3], lumas[5])) - centre_luma;
    float ridge_down = centre_luma - max(max(lumas[1], lumas[7]), max(lumas[3], lumas[5]));
    bool thin = max(ridge_up, ridge_down) > LOCK_CONTRAST;

    float moved = length(velocity * display_size);

    float lock = valid ? max(previous_meta.y - LOCK_DECAY, 0.0) : 0.0;
    float lock_luma = previous_meta.w;
    if (lock > 0.0 && abs(current_shape.x - lock_luma) > LOCK_DRIFT) {
        lock = 0.0;
    }
    if (thin && reactive < REACTIVE_LOCK) {
        lock = 1.0;
        lock_luma = current_shape.x;
    }
    float flicker = valid
        ? saturate(abs(current_shape.x - previous_meta.z) / max(high.x - low.x, 1e-3) - 0.5)
        : 0.0;

    float gamma = max(MOVING_GAMMA, lock > 0.0 ? LOCK_GAMMA : 0.0) + FLICKER_GAMMA * flicker;
    float limit = push.limit;
    if (edge && moved > MOVING_PIXELS) {
        gamma = min(gamma, EDGE_GAMMA);
        limit = min(limit, EDGE_LIMIT);
    }
    float4 mean = first / 9.0;
    float4 deviation = sqrt(max(second / 9.0 - mean * mean, 0.0));
    float4 box_low = min(mean - gamma * deviation, current_shape);
    float4 box_high = max(mean + gamma * deviation, current_shape);

    float4 result = current;
    if (accumulated > 0.0) {
        float4 history = ycocg(perceptual(catmull_rom(push.history, previous_uv, display_size), exposure));
        if (!still) {
            float4 clipped = clip_toward(box_low, box_high, history);
            float change = saturate(length(clipped - history) / max(length(box_high - box_low), CHANGE_FLOOR));
            accumulated = min(accumulated, lerp(limit, CHANGED_LIMIT, change));
            accumulated *= 1.0 - reactive;
            frame_weight *= lerp(1.0, 0.5, flicker);
            history = clipped;
        }
        float total = accumulated + frame_weight;
        float blend = total > WEIGHT_FLOOR ? frame_weight / total : 1.0;
        result = lerp(rgb_of(history), current, blend);
    }
    float kept = min(accumulated + frame_weight, limit);

    rikka_storage_rgba16f[NonUniformResourceIndex(push.output)][pixel] = linear_of(result, exposure);
    rikka_storage_rgba16f[NonUniformResourceIndex(push.output_meta)][pixel] =
        float4(kept, lock, current_shape.x, lock_luma);
}
