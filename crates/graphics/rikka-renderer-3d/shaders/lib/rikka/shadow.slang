__exported import rikka.common;
import rikka.traced;

struct RikkaShadowPoint {
    float3 world_position;
    float3 geometric_normal;
    float view_depth;
    uint flags;
    float rotation;
    uint traced;
};

struct RikkaShadowNoise {
    float2 spin;
    float2 dither;
};

static const float2 RIKKA_SHADOW_DETAIL_EDGE = float2(0.8, 0.95);

float rikka_shadow_rotation(RikkaFrameRef frame, float2 frag_xy) {
    uint seed = frame.view.shadow_noise;
    if (seed == 0u) {
        return 0.0;
    }
    return rikka_interleaved_gradient(frag_xy + 5.588238 * float(seed)) * RIKKA_TAU;
}

RikkaShadowNoise rikka_shadow_noise(RikkaShadowPoint p) {
    RikkaShadowNoise noise;
    noise.spin = float2(cos(p.rotation), sin(p.rotation));
    noise.dither = p.rotation == 0.0 ? float2(0.0) : 0.5 * noise.spin;
    return noise;
}

float2 rikka_shadow_spun(float2 tap, RikkaShadowNoise noise) {
    return float2(tap.x * noise.spin.x - tap.y * noise.spin.y, tap.x * noise.spin.y + tap.y * noise.spin.x);
}

struct RikkaShadowSample {
    float visibility;
    float blocker;
    float penumbra;
    float receiver;
    float3 tint;
    bool blocked;
};

RikkaShadowSample rikka_shadow_lit() {
    RikkaShadowSample s;
    s.visibility = 1.0;
    s.blocker = 0.0;
    s.penumbra = 0.0;
    s.receiver = 0.0;
    s.tint = float3(1.0);
    s.blocked = false;
    return s;
}

RikkaShadowSample rikka_shadow_combine(RikkaShadowSample a, RikkaShadowSample b) {
    RikkaShadowSample s = a;
    s.visibility = a.visibility * b.visibility;
    s.tint = a.tint * b.tint;
    if (b.blocked && (!a.blocked || b.blocker < a.blocker)) {
        s.blocker = b.blocker;
        s.penumbra = b.penumbra;
        s.receiver = b.receiver;
    }
    s.blocked = a.blocked || b.blocked;
    return s;
}

RikkaShadowSample rikka_shadow_mix(RikkaShadowSample a, RikkaShadowSample b, float t) {
    RikkaShadowSample s = t < 0.5 ? a : b;
    s.visibility = lerp(a.visibility, b.visibility, t);
    s.tint = lerp(a.tint, b.tint, t);
    return s;
}

static const float2 RIKKA_SHADOW_DISC[16] = {
    float2(-0.94201624, -0.39906216), float2(0.94558609, -0.76890725),
    float2(-0.09418410, -0.92938870), float2(0.34495938, 0.29387760),
    float2(-0.91588581, 0.45771432), float2(-0.81544232, -0.87912464),
    float2(-0.38277543, 0.27676845), float2(0.97484398, 0.75648379),
    float2(0.44323325, -0.97511554), float2(0.53742981, -0.47373420),
    float2(-0.26496911, -0.41893023), float2(0.79197514, 0.19090188),
    float2(-0.24188840, 0.99706507), float2(-0.81409955, 0.91437590),
    float2(0.19984126, 0.78641367), float2(0.14383161, -0.14100790) };

struct RikkaShadowTile {
    float2 uv;
    float reference;
    float2 lowest;
    float2 highest;
    float atlas_size;
    uint image;
};

bool rikka_shadow_tile(RikkaFrameRef frame, RikkaShadowSlot slot, float3 world_position, out RikkaShadowTile tile) {
    tile = {};
    float4 clip = mul(slot.view_projection, float4(world_position, 1.0));
    if (clip.w <= 0.0) {
        return false;
    }
    float3 ndc = clip.xyz / clip.w;
    if (any(abs(ndc.xy) > float2(1.0))) {
        return false;
    }
    if (ndc.z <= 0.0) {
        if ((slot.flags & RIKKA_SHADOW_FIGURE) == 0u) {
            return false;
        }
        ndc.z = 1e-7;
    }
    tile.uv = (ndc.xy * 0.5 + 0.5) * slot.uv_scale + slot.uv_offset;
    tile.reference = ndc.z;
    tile.lowest = slot.uv_offset + 0.5 * slot.texel_size;
    tile.highest = slot.uv_offset + slot.uv_scale - 0.5 * slot.texel_size;
    tile.atlas_size = 1.0 / slot.texel_size;
    tile.image = (slot.flags & RIKKA_SHADOW_FIGURE) != 0u ? frame.view.figure_atlas : frame.view.shadow_atlas;
    return true;
}

float rikka_shadow_compare(uint image, float2 uv, float reference, float2 lowest, float2 highest) {
    return rikka_images[NonUniformResourceIndex(image)].SampleCmpLevelZero(
        rikka_shadow_samplers[RIKKA_SAMPLER_SHADOW],
        clamp(uv, lowest, highest),
        reference);
}

float rikka_shadow_depth(RikkaShadowTile tile, float2 uv) {
    return rikka_fetch(rikka_images[NonUniformResourceIndex(tile.image)], int2(uv * tile.atlas_size), 0).r;
}

float3 rikka_shadow_tint(RikkaFrameRef frame, RikkaShadowSlot slot, RikkaShadowTile tile) {
    RikkaViewRef view = frame.view;
    if ((slot.flags & RIKKA_SHADOW_TINTED) == 0u || view.shadow_tint == RIKKA_NONE) {
        return float3(1.0);
    }
    float2 uv = clamp(tile.uv, tile.lowest, tile.highest);
    float in_front = rikka_images[view.shadow_tint_depth].SampleCmpLevelZero(
        rikka_shadow_samplers[RIKKA_SAMPLER_SHADOW], uv, tile.reference);
    float3 tint = rikka_images[view.shadow_tint].SampleLevel(rikka_samplers[1], uv, 0.0).rgb;
    return lerp(tint, float3(1.0), in_front);
}

float rikka_shadow_filtered(RikkaFrameRef frame, RikkaShadowTile tile, float reference) {
    float2 at = tile.uv * tile.atlas_size;
    float2 base = floor(at + 0.5);
    float s = at.x + 0.5 - base.x;
    float t = at.y + 0.5 - base.y;
    base = (base - 0.5) / tile.atlas_size;
    float3 uw = float3(4.0 - 3.0 * s, 7.0, 1.0 + 3.0 * s);
    float3 u = float3((3.0 - 2.0 * s) / uw.x - 2.0, (3.0 + s) / uw.y, s / uw.z + 2.0);
    float3 vw = float3(4.0 - 3.0 * t, 7.0, 1.0 + 3.0 * t);
    float3 v = float3((3.0 - 2.0 * t) / vw.x - 2.0, (3.0 + t) / vw.y, t / vw.z + 2.0);
    float sum = 0.0;
    [unroll]
    for (int y = 0; y < 3; ++y) {
        [unroll]
        for (int x = 0; x < 3; ++x) {
            float2 uv = base + float2(u[x], v[y]) / tile.atlas_size;
            sum += uw[x] * vw[y] * rikka_shadow_compare(tile.image, uv, reference, tile.lowest, tile.highest);
        }
    }
    return sum * (1.0 / 144.0);
}

RikkaShadowSample rikka_shadow_hard_tap(RikkaFrameRef frame, RikkaShadowSlot slot, float3 world_position, float3 normal) {
    RikkaShadowSample s = rikka_shadow_lit();
    RikkaShadowTile tile;
    if (!rikka_shadow_tile(frame, slot, world_position + normal * slot.normal_bias, tile)) {
        return s;
    }
    s.visibility = rikka_shadow_compare(tile.image, tile.uv, tile.reference + slot.depth_bias, tile.lowest, tile.highest);
    s.blocked = s.visibility < 1.0;
    s.tint = rikka_shadow_tint(frame, slot, tile);
    return s;
}

RikkaShadowSample rikka_shadow_pcf(RikkaFrameRef frame, RikkaShadowSlot slot, float3 world_position, float3 normal, float3 to_light, RikkaShadowNoise noise) {
    RikkaShadowSample s = rikka_shadow_lit();
    float n_dot_l = clamp(dot(normal, to_light), -1.0, 1.0);
    float sine = sqrt(max(1.0 - n_dot_l * n_dot_l, 0.0));
    float3 moved = world_position + normal * (slot.normal_bias * (0.4 + 0.8 * sine));
    RikkaShadowTile tile;
    if (!rikka_shadow_tile(frame, slot, moved, tile)) {
        return s;
    }
    tile.uv += noise.dither * slot.texel_size;
    s.visibility = rikka_shadow_filtered(frame, tile, tile.reference + slot.depth_bias);
    s.blocked = s.visibility < 1.0;
    s.tint = rikka_shadow_tint(frame, slot, tile);
    return s;
}

RikkaShadowNoise rikka_shadow_still() {
    RikkaShadowNoise noise;
    noise.spin = float2(1.0, 0.0);
    noise.dither = float2(0.0);
    return noise;
}

float rikka_shadow_lookup(RikkaFrameRef frame, RikkaShadowSlot slot, float3 world_position, float3 normal) {
    return rikka_shadow_pcf(frame, slot, world_position, normal, normal, rikka_shadow_still()).visibility;
}

RikkaShadowSample rikka_shadow_sun(RikkaFrameRef frame, RikkaShadowSlot slot, float3 world_position, float3 normal, float3 to_light, float angular_radius, RikkaShadowNoise noise) {
    RikkaShadowSample s = rikka_shadow_lit();
    float texel = slot.world_texel;
    float n_dot_l = clamp(dot(normal, to_light), -1.0, 1.0);
    float sine = sqrt(max(1.0 - n_dot_l * n_dot_l, 0.0));
    float slope = min(sine / max(abs(n_dot_l), 0.05), 6.0);
    float3 moved = world_position
        + normal * (texel * (0.6 + 1.4 * sine))
        + to_light * (texel * (0.4 + 0.6 * slope));
    RikkaShadowTile tile;
    if (!rikka_shadow_tile(frame, slot, moved, tile)) {
        return s;
    }
    float world_per_uv = texel * tile.atlas_size;
    float range = slot.depth_range;
    float spread = max(angular_radius, 0.0);
    s.receiver = (1.0 - tile.reference) * range;
    s.tint = rikka_shadow_tint(frame, slot, tile);

    float search = clamp(spread * 3.0 / world_per_uv, 1.5 * slot.texel_size, 16.0 * slot.texel_size);
    float blockers = 0.0;
    float blocker_depth = 0.0;
    [unroll]
    for (int i = 0; i < 16; ++i) {
        float2 uv = clamp(tile.uv + rikka_shadow_spun(RIKKA_SHADOW_DISC[i], noise) * search, tile.lowest, tile.highest);
        float depth = rikka_shadow_depth(tile, uv);
        if (depth > tile.reference) {
            blockers += 1.0;
            blocker_depth += depth;
        }
    }
    if (blockers == 0.0) {
        return s;
    }
    float distance = max(blocker_depth / blockers - tile.reference, 0.0) * range;
    float radius = clamp(distance * spread / world_per_uv, 1.5 * slot.texel_size, 24.0 * slot.texel_size);
    float reference = tile.reference + radius * world_per_uv * min(slope, 3.0) / range;

    float sum = 0.0;
    [unroll]
    for (int i = 0; i < 16; ++i) {
        sum += rikka_shadow_compare(tile.image, tile.uv + rikka_shadow_spun(RIKKA_SHADOW_DISC[i], noise) * radius, reference, tile.lowest, tile.highest);
    }
    s.visibility = sum * (1.0 / 16.0);
    s.blocked = true;
    s.blocker = distance;
    s.penumbra = 2.0 * distance * spread;
    return s;
}

float rikka_shadow_linear(RikkaShadowSlot slot, float depth) {
    return slot.near / max(depth, 1e-9);
}

RikkaShadowSample rikka_shadow_local_soft(RikkaFrameRef frame, RikkaShadowSlot slot, float3 world_position, float3 normal, float3 to_light, float size, RikkaShadowNoise noise) {
    RikkaShadowSample s = rikka_shadow_lit();
    float n_dot_l = clamp(dot(normal, to_light), -1.0, 1.0);
    float sine = sqrt(max(1.0 - n_dot_l * n_dot_l, 0.0));
    float3 moved = world_position + normal * (slot.normal_bias * (0.4 + 0.8 * sine));
    RikkaShadowTile tile;
    if (!rikka_shadow_tile(frame, slot, moved, tile)) {
        return s;
    }
    float reference = tile.reference + slot.depth_bias;
    s.tint = rikka_shadow_tint(frame, slot, tile);
    float receiver = rikka_shadow_linear(slot, tile.reference);
    s.receiver = receiver;
    float uv_per_world = 0.5 * slot.projection_scale * slot.uv_scale.x / receiver;
    if (size <= 0.0) {
        tile.uv += noise.dither * slot.texel_size;
        s.visibility = rikka_shadow_filtered(frame, tile, reference);
        s.blocked = s.visibility < 1.0;
        return s;
    }
    float search = clamp(2.0 * size * uv_per_world, 1.5 * slot.texel_size, 16.0 * slot.texel_size);
    float blockers = 0.0;
    float blocker_depth = 0.0;
    [unroll]
    for (int i = 0; i < 16; ++i) {
        float2 uv = clamp(tile.uv + rikka_shadow_spun(RIKKA_SHADOW_DISC[i], noise) * search, tile.lowest, tile.highest);
        float depth = rikka_shadow_depth(tile, uv);
        if (depth > reference) {
            blockers += 1.0;
            blocker_depth += rikka_shadow_linear(slot, depth);
        }
    }
    if (blockers == 0.0) {
        return s;
    }
    float blocker = blocker_depth / blockers;
    float penumbra = size * max(receiver - blocker, 0.0) / max(blocker, 1e-4);
    float radius = clamp(penumbra * uv_per_world, 1.5 * slot.texel_size, 24.0 * slot.texel_size);
    float sum = 0.0;
    [unroll]
    for (int i = 0; i < 16; ++i) {
        sum += rikka_shadow_compare(tile.image, tile.uv + rikka_shadow_spun(RIKKA_SHADOW_DISC[i], noise) * radius, reference, tile.lowest, tile.highest);
    }
    s.visibility = sum * (1.0 / 16.0);
    s.blocked = true;
    s.blocker = max(receiver - blocker, 0.0);
    s.penumbra = penumbra;
    return s;
}

uint rikka_cascade(RikkaFrameRef frame, RikkaLight light, float view_depth) {
    RikkaShadowRef shadows = frame.shadows;
    for (uint i = 0u; i + 1u < light.shadow_count; ++i) {
        if (view_depth < shadows[light.shadow + i].split_depth) {
            return i;
        }
    }
    return light.shadow_count - 1u;
}

uint rikka_cube_face(float3 direction) {
    float3 a = abs(direction);
    if (a.x >= a.y && a.x >= a.z) {
        return direction.x >= 0.0 ? 0u : 1u;
    }
    if (a.y >= a.z) {
        return direction.y >= 0.0 ? 2u : 3u;
    }
    return direction.z >= 0.0 ? 4u : 5u;
}

RikkaShadowSample rikka_shadow_directional_view(RikkaFrameRef frame, RikkaShadowSlot slot, RikkaShadowPoint p, float3 to_light, float angular_radius, bool soft) {
    RikkaShadowNoise noise = rikka_shadow_noise(p);
    if (soft) {
        return rikka_shadow_sun(frame, slot, p.world_position, p.geometric_normal, to_light, angular_radius, noise);
    }
    return rikka_shadow_pcf(frame, slot, p.world_position, p.geometric_normal, to_light, noise);
}

float rikka_shadow_detail_reach(RikkaShadowSlot detail, float3 world_position) {
    float4 clip = mul(detail.view_projection, float4(world_position, 1.0));
    return max(abs(clip.x), abs(clip.y)) / clip.w;
}

uint rikka_shadow_figure_views(RikkaShadowSlot slot) {
    return (slot.flags & RIKKA_SHADOW_DETAILED) != 0u ? 2u : 1u;
}

uint rikka_shadow_figure_pick(RikkaFrameRef frame, uint index, float3 world_position) {
    if ((frame.shadows[index].flags & RIKKA_SHADOW_DETAILED) == 0u) {
        return index;
    }
    return rikka_shadow_detail_reach(frame.shadows[index + 1u], world_position) < RIKKA_SHADOW_DETAIL_EDGE.y ? index + 1u : index;
}

RikkaShadowSample rikka_shadow_figure(RikkaFrameRef frame, uint index, RikkaShadowPoint p, float3 to_light, float angular_radius, bool soft) {
    RikkaShadowSlot body = frame.shadows[index];
    if ((body.flags & RIKKA_SHADOW_DETAILED) == 0u) {
        return rikka_shadow_directional_view(frame, body, p, to_light, angular_radius, soft);
    }
    RikkaShadowSlot detail = frame.shadows[index + 1u];
    float blend = smoothstep(RIKKA_SHADOW_DETAIL_EDGE.x, RIKKA_SHADOW_DETAIL_EDGE.y, rikka_shadow_detail_reach(detail, p.world_position));
    if (blend >= 1.0) {
        return rikka_shadow_directional_view(frame, body, p, to_light, angular_radius, soft);
    }
    RikkaShadowSample close = rikka_shadow_directional_view(frame, detail, p, to_light, angular_radius, soft);
    if (blend <= 0.0) {
        return close;
    }
    return rikka_shadow_mix(close, rikka_shadow_directional_view(frame, body, p, to_light, angular_radius, soft), blend);
}

RikkaShadowSample rikka_shadow_cascaded(RikkaFrameRef frame, RikkaLight light, RikkaShadowPoint p, float3 to_light, bool soft) {
    RikkaShadowRef shadows = frame.shadows;
    if ((shadows[light.shadow].flags & RIKKA_SHADOW_FIGURE) != 0u) {
        return rikka_shadow_figure(frame, light.shadow, p, to_light, light.size, soft);
    }
    uint cascade = rikka_cascade(frame, light, p.view_depth);
    RikkaShadowSlot slot = shadows[light.shadow + cascade];
    float start = cascade == 0u ? 0.0 : shadows[light.shadow + cascade - 1u].split_depth;
    float along = (p.view_depth - start) / max(slot.split_depth - start, 1e-4);
    RikkaShadowSample value = rikka_shadow_directional_view(frame, slot, p, to_light, light.size, soft);
    float blend = smoothstep(0.82, 1.0, along);
    if (blend <= 0.0) {
        return value;
    }
    if (cascade + 1u < light.shadow_count) {
        RikkaShadowSample next = rikka_shadow_directional_view(frame, shadows[light.shadow + cascade + 1u], p, to_light, light.size, soft);
        return rikka_shadow_mix(value, next, blend);
    }
    return rikka_shadow_mix(value, rikka_shadow_lit(), blend);
}

RikkaShadowSample rikka_shadow_figures(RikkaFrameRef frame, RikkaLight light, RikkaShadowPoint p, float3 to_light, bool soft) {
    RikkaShadowSample s = rikka_shadow_lit();
    uint i = 0u;
    while (i < light.character_count) {
        uint index = light.characters + i;
        s = rikka_shadow_combine(s, rikka_shadow_figure(frame, index, p, to_light, light.size, soft));
        i += rikka_shadow_figure_views(frame.shadows[index]);
    }
    return s;
}

bool rikka_shadow_mapped(RikkaLight light) {
    return light.shadow < RIKKA_SHADOW_UNMAPPED;
}

bool rikka_shadow_traced(RikkaFrameRef frame, RikkaLight light, RikkaShadowPoint p, out RikkaShadowSample s) {
    s = rikka_shadow_lit();
    if (light.shadow == RIKKA_NONE) {
        return false;
    }
    float traced = rikka_traced_shadow(frame, light.shadow, p.traced, p.world_position, p.view_depth);
    if (traced < 0.0) {
        return false;
    }
    s.visibility = traced;
    s.blocked = traced < 1.0;
    return true;
}

RikkaShadowSample rikka_shadow_evaluate(RikkaFrameRef frame, RikkaLight light, RikkaShadowPoint p) {
    if ((p.flags & RIKKA_RECEIVES_SHADOW) == 0u) {
        return rikka_shadow_lit();
    }
    RikkaShadowSample traced;
    if (rikka_shadow_traced(frame, light, p, traced)) {
        return traced;
    }
    bool soft = frame.look.scene.enabled != 0u;
    if (light.kind == RIKKA_LIGHT_DIRECTIONAL) {
        float3 to_light = -light.direction;
        RikkaShadowSample s = rikka_shadow_mapped(light)
            ? rikka_shadow_cascaded(frame, light, p, to_light, soft)
            : rikka_shadow_lit();
        if (light.character_count > 0u) {
            s = rikka_shadow_combine(s, rikka_shadow_figures(frame, light, p, to_light, soft));
        }
        return s;
    }
    if (!rikka_shadow_mapped(light)) {
        return rikka_shadow_lit();
    }
    float3 to_light = normalize(light.position - p.world_position);
    uint face = light.kind == RIKKA_LIGHT_SPOT ? 0u : rikka_cube_face(p.world_position - light.position);
    RikkaShadowSlot slot = frame.shadows[light.shadow + face];
    return rikka_shadow_local_soft(frame, slot, p.world_position, p.geometric_normal, to_light, soft ? light.size : 0.0, rikka_shadow_noise(p));
}

float rikka_shadow(RikkaFrameRef frame, RikkaLight light, RikkaShadowPoint p) {
    return rikka_shadow_evaluate(frame, light, p).visibility;
}

float rikka_shadow_hard(RikkaFrameRef frame, RikkaLight light, RikkaShadowPoint p) {
    if ((p.flags & RIKKA_RECEIVES_SHADOW) == 0u) {
        return 1.0;
    }
    RikkaShadowSample traced;
    if (rikka_shadow_traced(frame, light, p, traced)) {
        return traced.visibility;
    }
    float visibility = 1.0;
    if (rikka_shadow_mapped(light)) {
        uint index = light.shadow;
        if (light.kind == RIKKA_LIGHT_DIRECTIONAL) {
            index = (frame.shadows[light.shadow].flags & RIKKA_SHADOW_FIGURE) != 0u
                ? rikka_shadow_figure_pick(frame, light.shadow, p.world_position)
                : light.shadow + rikka_cascade(frame, light, p.view_depth);
        } else if (light.kind != RIKKA_LIGHT_SPOT) {
            index += rikka_cube_face(p.world_position - light.position);
        }
        visibility = rikka_shadow_hard_tap(frame, frame.shadows[index], p.world_position, p.geometric_normal).visibility;
    }
    uint i = 0u;
    while (i < light.character_count) {
        uint index = light.characters + i;
        visibility *= rikka_shadow_hard_tap(frame, frame.shadows[rikka_shadow_figure_pick(frame, index, p.world_position)], p.world_position, p.geometric_normal).visibility;
        i += rikka_shadow_figure_views(frame.shadows[index]);
    }
    return visibility;
}
