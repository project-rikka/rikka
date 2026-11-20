__exported import rikka.emitter.push;
import rikka.surface;
import rikka.temporal;
import rikka.space.view;

static const uint LOOK_SOFT = 0u;
static const uint LOOK_DISC = 1u;
static const uint LOOK_STREAK = 2u;
static const uint LOOK_STAR = 3u;
static const uint LOOK_PETAL = 4u;
static const uint LOOK_LEAF = 5u;
static const uint LOOK_FLAKE = 6u;
static const uint LOOK_SQUARE = 7u;

static const float SMALLEST_PIXELS = 1.0;
static const float NEAR_FADE = 0.1;
static const float SOFTEST = 1e-3;
static const float FAINTEST = 1.0 / 1024.0;

static const float2 CORNERS[6] = {
    float2(-1.0, -1.0), float2(1.0, -1.0), float2(1.0, 1.0),
    float2(-1.0, -1.0), float2(1.0, 1.0), float2(-1.0, 1.0) };

struct EmitterVaryings {
    [[vk::location(0)]] float2 corner;
    [[vk::location(1)]] float2 uv;
    [[vk::location(2)]] nointerpolation float3 lit;
    [[vk::location(3)]] nointerpolation float3 emitted;
    [[vk::location(4)]] nointerpolation float opacity;
    [[vk::location(5)]] nointerpolation float distance;
    [[vk::location(6)]] nointerpolation float softness;
    [[vk::location(7)]] nointerpolation float slender;
    [[vk::location(8)]] float4 now;
    [[vk::location(9)]] float4 then;
};

float2 unpack_halves(uint packed) {
    return float2(f16tof32(packed & 0xffffu), f16tof32(packed >> 16u));
}

float3 spin_about(float3 v, float3 axis, float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return v * c + cross(axis, v) * s + axis * dot(axis, v) * (1.0 - c);
}

float3 across_of(float3 along, float3 eye, float3 fallback) {
    float3 side = cross(along, eye);
    float reach = length(side);
    return reach > 1e-5 ? side / reach : fallback;
}

EmitterVaryings rikka_emitter_vertex(uint vertex_index, uint instance, out float4 position) {
    EmitterVaryings out = {};
    position = float4(-2.0, -2.0, 0.0, 1.0);
    RikkaEmitterRef emitter = rikka_pushed_emitter();
    RikkaEmitterParticle particle = rikka_emitter_push.particles[emitter.first_particle + instance];
    if (!(particle.size > 0.0) || !(particle.opacity > 0.0)) {
        return out;
    }
    RikkaFrameRef frame = rikka_emitter_push.frame;
    RikkaViewRef view = frame.view;
    float2 corner = CORNERS[vertex_index % 6u];
    float3 right = float3(view.view[0][0], view.view[0][1], view.view[0][2]);
    float3 up = float3(view.view[1][0], view.view[1][1], view.view[1][2]);
    float3 centre = particle.position;
    float3 eye = normalize(centre - view.camera_position);
    float half_length = 0.5 * particle.size;
    float half_width = 0.5 * particle.size * max(emitter.aspect, 0.0);
    float angle = particle.rotation;
    float c = cos(angle);
    float s = sin(angle);
    float speed = length(particle.velocity);
    bool stretched = emitter.stretch > 0.0 && speed > 1e-4;
    float3 across;
    float3 along;
    uint facing = emitter.facing;
    if (facing == RIKKA_FACING_VELOCITY || (facing == RIKKA_FACING_CAMERA && stretched)) {
        float3 heading = speed > 1e-5 ? particle.velocity / speed : up;
        float3 flat = heading - eye * dot(heading, eye);
        float reach = length(flat);
        along = reach > 1e-5 ? flat / reach : up;
        across = across_of(along, eye, right);
        if (stretched) {
            half_length += 0.5 * clamp(speed * emitter.stretch, emitter.stretch_min, emitter.stretch_max);
        }
    } else if (facing == RIKKA_FACING_HORIZONTAL) {
        across = float3(c, 0.0, s);
        along = float3(-s, 0.0, c);
    } else if (facing == RIKKA_FACING_AXIS) {
        float reach = length(emitter.axis);
        along = reach > 1e-5 ? emitter.axis / reach : float3(0.0, 1.0, 0.0);
        across = across_of(along, eye, right);
    } else if (facing == RIKKA_FACING_TUMBLE) {
        uint hash = particle.shade.w & 0xffffu;
        float y = float(hash & 0xffu) / 255.0 * 2.0 - 1.0;
        float phi = float(hash >> 8u) / 255.0 * RIKKA_TAU;
        float reach = sqrt(max(1.0 - y * y, 0.0));
        float3 axis = float3(reach * cos(phi), y, reach * sin(phi));
        across = spin_about(float3(1.0, 0.0, 0.0), axis, angle);
        along = spin_about(float3(0.0, 1.0, 0.0), axis, angle);
    } else {
        across = right * c + up * s;
        along = up * c - right * s;
    }

    float opacity = particle.opacity;
    float4 centre_clip = mul(view.view_projection, float4(centre, 1.0));
    float pixels = abs(view.projection[1][1]) * 0.5 * view.viewport.y / max(centre_clip.w, 1e-4);
    float width_pixels = 2.0 * half_width * pixels;
    if (width_pixels < SMALLEST_PIXELS) {
        float grow = SMALLEST_PIXELS / max(width_pixels, 1e-3);
        half_width *= grow;
        opacity /= grow;
    }
    float length_pixels = 2.0 * half_length * pixels;
    if (length_pixels < SMALLEST_PIXELS) {
        float grow = SMALLEST_PIXELS / max(length_pixels, 1e-3);
        half_length *= grow;
        opacity /= grow;
    }

    float3 offset = across * (corner.x * half_width) + along * (corner.y * half_length);
    float3 world = centre + offset;
    position = mul(view.view_projection, float4(world, 1.0));

    uint frame_index = particle.shade.w >> 16u;
    uint columns = max(emitter.columns, 1u);
    uint rows = max(emitter.rows, 1u);
    float2 cell = float2(float(frame_index % columns), float(frame_index / columns));
    float2 inside = float2(0.5 + 0.5 * corner.x, 0.5 - 0.5 * corner.y);

    float2 first = unpack_halves(particle.shade.x);
    float2 second = unpack_halves(particle.shade.y);
    float2 third = unpack_halves(particle.shade.z);
    out.corner = corner;
    out.uv = (cell + inside) / float2(float(columns), float(rows));
    out.lit = float3(first, second.x);
    out.emitted = float3(second.y, third);
    out.opacity = opacity;
    out.distance = max(-mul(view.view, float4(centre, 1.0)).z, 0.0);
    out.softness = max(emitter.softness, SOFTEST);
    out.slender = half_width / max(half_length, 1e-6);
    out.now = rikka_unjittered(view, position);
    out.then = mul(frame.look.scene.previous_view_projection, float4(particle.previous + offset, 1.0));
    return out;
}

float edge_coverage(float field) {
    float soft = max(fwidth(field), 1e-4);
    return clamp(0.5 - field / soft, 0.0, 1.0);
}

float petal_shape(float2 c) {
    float y = c.y;
    float width = sqrt(max(1.0 - y * y, 0.0)) * (0.5 + 0.5 * (y + 1.0) * 0.5);
    float body = abs(c.x) - width;
    float notch = (0.8 + abs(c.x) * 0.9) - y;
    return edge_coverage(max(body, -notch));
}

float star_shape(float2 c) {
    float2 a = abs(c);
    float rays = exp(-a.x * 14.0) * exp(-a.y * 2.2) + exp(-a.y * 14.0) * exp(-a.x * 2.2);
    float core = exp(-dot(c, c) * 18.0);
    return clamp(rays * 0.8 + core, 0.0, 1.0) * (1.0 - smoothstep(0.85, 1.0, length(c)));
}

float2 leaf_shape(float2 c) {
    float y = c.y;
    float width = sqrt(max(1.0 - y * y, 0.0)) * (1.0 - 0.25 * y);
    float body = edge_coverage(abs(c.x) - width);
    float vein = 1.0 - 0.3 * exp(-c.x * c.x * 400.0) * step(y, 0.9);
    return float2(body, vein);
}

float flake_shape(float2 c) {
    float r = length(c);
    float turn = atan2(c.y, c.x);
    float arms = abs(cos(3.0 * turn));
    float arms2 = arms * arms;
    float reach = r / (0.55 + 0.45 * arms2 * arms2 * arms2);
    return 1.0 - smoothstep(0.7, 1.0, reach);
}

float streak_shape(float2 c, float slender) {
    float ratio = 1.0 / max(slender, 1e-3);
    float y = abs(c.y) * ratio;
    float d = length(float2(c.x, max(y - (ratio - 1.0), 0.0)));
    float body = saturate(1.0 - d);
    return body * body;
}

float2 procedural(uint look, float2 c, float slender) {
    float r = length(c);
    switch (look) {
    case LOOK_DISC:
        return float2(edge_coverage(r - 1.0), 1.0);
    case LOOK_STREAK:
        return float2(streak_shape(c, slender), 1.0);
    case LOOK_STAR:
        return float2(star_shape(c), 1.0);
    case LOOK_PETAL: {
        float base = smoothstep(-0.9, 0.5, c.y);
        return float2(petal_shape(c), lerp(1.25, 1.0, base));
    }
    case LOOK_LEAF:
        return leaf_shape(c);
    case LOOK_FLAKE:
        return float2(flake_shape(c), 1.0);
    case LOOK_SQUARE: {
        float edge = max(abs(c.x), abs(c.y)) - 1.0;
        return float2(edge_coverage(edge), 1.0);
    }
    default:
        return float2(max(exp(-r * r * 4.0) - exp(-4.0), 0.0) / (1.0 - exp(-4.0)), 1.0);
    }
}

struct EmitterSample {
    float4 color;
    float4 emission;
    float weight;
};

EmitterSample rikka_emitter_fragment(EmitterVaryings varyings, float4 frag_coord) {
    EmitterSample result = {};
    RikkaEmitterRef emitter = rikka_pushed_emitter();
    RikkaFrameRef frame = rikka_emitter_push.frame;
    RikkaScene scene = frame.look.scene;

    float soft = 1.0;
    float depth = rikka_read_depth(scene.depth, int2(frag_coord.xy));
    if (depth > 0.0) {
        float behind = space_distance_of(frame.view.projection, depth) - varyings.distance;
        soft = clamp(behind / varyings.softness, 0.0, 1.0);
    }
    float near = frame.view.near;
    soft *= smoothstep(near, near + NEAR_FADE, varyings.distance);
    float strength = varyings.opacity * soft;

    float3 radiance = varyings.lit + varyings.emitted;
    float3 glow = emitter.bloom != 0u ? varyings.emitted : float3(0.0);
    float alpha;
    float3 color;
    float3 emission;
    if (emitter.sprite != RIKKA_NONE) {
        float4 texel = rikka_images[NonUniformResourceIndex(emitter.sprite)].Sample(
            rikka_samplers[NonUniformResourceIndex(emitter.sampler)], varyings.uv);
        if (emitter.blend == RIKKA_PARTICLE_BLEND_PREMULTIPLIED) {
            alpha = texel.a * strength;
            color = radiance * texel.rgb * strength;
            emission = glow * texel.rgb * strength;
        } else {
            alpha = texel.a * strength;
            color = radiance * texel.rgb * alpha;
            emission = glow * texel.rgb * alpha;
        }
    } else {
        float2 shape = procedural(emitter.look, varyings.corner, varyings.slender);
        alpha = shape.x * strength;
        color = radiance * shape.y * alpha;
        emission = glow * shape.y * alpha;
    }
    float covered = emitter.blend == RIKKA_PARTICLE_BLEND_ADDITIVE ? 0.0 : alpha;
    result.color = float4(color, covered);
    result.emission = float4(emission, covered);
    result.weight = alpha;
    return result;
}

bool rikka_emitter_faint(EmitterSample sample) {
    return sample.weight < FAINTEST;
}

float2 rikka_emitter_motion(EmitterVaryings varyings) {
    return rikka_motion(varyings.now, varyings.then);
}
