import rikka.push;
import rikka.fragment;
import rikka.space.view;
import rikka.particles;
import rikka.sky;

struct ParticleVaryings {
    [[vk::location(0)]] float2 corner;
    [[vk::location(1)]] float3 light;
    [[vk::location(2)]] float3 glow;
    [[vk::location(3)]] float opacity;
    [[vk::location(4)]] float distance;
    [[vk::location(5)]] float size;
    [[vk::location(6)]] nointerpolation uint kind;
};

static const float2 CORNERS[6] = {
    float2(-1.0, -1.0), float2(1.0, -1.0), float2(1.0, 1.0),
    float2(-1.0, -1.0), float2(1.0, 1.0), float2(-1.0, 1.0) };

static const float SIZES[4] = { 0.012, 0.028, 0.022, 0.014 };
static const float3 FALLS[4] = {
    float3(0.0, 0.01, 0.0), float3(0.0, -0.45, 0.0), float3(0.0, 0.03, 0.0), float3(0.0, -0.7, 0.0) };
static const float SWAYS[4] = { 0.12, 0.35, 0.05, 0.22 };
static const float GLOWS[4] = { 1.2, 0.08, 5.0, 0.25 };
static const float OPACITIES[4] = { 0.7, 1.0, 0.35, 0.9 };
static const float SCATTERS[4] = { 2.5, 0.6, 1.0, 1.2 };
static const float SMALLEST_PIXELS = 1.5;
static const float ADDITIVE[4] = { 0.6, 1.0, 0.25, 1.0 };

float3 rotate(float3 v, float3 axis, float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return v * c + cross(axis, v) * s + axis * dot(axis, v) * (1.0 - c);
}

[shader("vertex")]
ParticleVaryings vertex_main(
    uint vertex_index: SV_VulkanVertexID,
    uint instance_index: SV_VulkanInstanceID,
    out float4 position: SV_Position)
{
    RikkaFrameRef frame = rikka_push.frame;
    RikkaViewRef view = frame.view;
    RikkaSpace space = frame.look.space;
    uint kind = min(space.particle_kind, 3u);
    uint index = instance_index;
    float2 corner = CORNERS[vertex_index % 6u];

    uint state = rikka_pcg_hash(index * 0x9E3779B9u ^ rikka_pcg_hash(space.particle_seed + 0x632BE5ABu));
    float3 home = float3(space_random(state), space_random(state), space_random(state));
    float pace = 0.7 + 0.6 * space_random(state);
    float3 phase = float3(space_random(state), space_random(state), space_random(state)) * 2.0 * RIKKA_PI;
    float spread = space_random(state);
    float flicker = space_random(state);
    float3 axis = normalize(float3(space_random(state), space_random(state), space_random(state)) - 0.5 + 1e-3);
    float spin = (space_random(state) - 0.5) * 6.0;

    float reach = max(space.particle_reach, 0.1);
    float time = view.time;
    float3 velocity = space.particle_wind + FALLS[kind] * pace;
    float3 sway = SWAYS[kind] * float3(
        sin(time * 0.9 * pace + phase.x),
        0.5 * sin(time * 0.7 * pace + phase.y),
        cos(time * 0.8 * pace + phase.z));
    float3 drift = home * reach + velocity * time + sway;
    float3 focus = view.camera_position + view.camera_forward * max(space.particle_focus, 0.0);
    float3 local = rikka_mod(drift - focus + 0.5 * reach, float3(reach)) - 0.5 * reach;
    float3 centre = focus + local;
    float edge = max(max(abs(local.x), abs(local.y)), abs(local.z)) / (0.5 * reach);
    float opacity = OPACITIES[kind] * (1.0 - smoothstep(0.8, 1.0, edge));

    float size = SIZES[kind] * max(space.particle_size, 0.0) * (0.6 + 0.8 * spread);
    float4 centre_clip = mul(view.view_projection, float4(centre, 1.0));
    float pixels = size * abs(view.projection[1][1]) * 0.5 * view.viewport.y / max(centre_clip.w, 1e-4);
    if (pixels < SMALLEST_PIXELS) {
        float grow = SMALLEST_PIXELS / max(pixels, 1e-3);
        size *= grow;
        opacity /= grow * grow;
    }

    float3 right = float3(view.view[0][0], view.view[0][1], view.view[0][2]);
    float3 up = float3(view.view[1][0], view.view[1][1], view.view[1][2]);
    float3 offset;
    if (kind == RIKKA_PARTICLE_PETALS) {
        float angle = phase.x + time * spin;
        float3 across = rotate(rotate(right, axis, angle), up, phase.y * 0.3);
        float3 along = rotate(rotate(up, axis, angle), up, phase.y * 0.3);
        offset = (across * corner.x * 0.7 + along * corner.y) * size;
    } else {
        offset = (right * corner.x + up * corner.y) * size;
    }
    float3 world = centre + offset;

    float3 sky = rikka_sky_specular(frame, float3(0.0, 1.0, 0.0), 1.0);
    float3 sun = float3(0.0);
    if (view.directional_count > 0u) {
        RikkaLight light = frame.lights[0];
        float3 seen = normalize(centre - view.camera_position);
        float forward = max(dot(seen, -light.direction), 0.0);
        float scatter = 0.35 + SCATTERS[kind] * pow(forward, 6.0);
        sun = light.color * light.intensity * scatter * 0.3;
    }
    float twinkle = kind == RIKKA_PARTICLE_SPARKLES
        ? 0.1 + 0.9 * pow(0.5 + 0.5 * sin(time * (2.0 + 4.0 * flicker) + phase.z), 4.0)
        : 0.75 + 0.25 * sin(time * (0.5 + flicker) + phase.z);

    ParticleVaryings out;
    out.corner = corner;
    out.light = sun + sky;
    out.glow = space.particle_color * space.particle_glow * GLOWS[kind] * twinkle;
    out.opacity = opacity;
    out.distance = max(-mul(view.view, float4(centre, 1.0)).z, 0.0);
    out.size = size;
    out.kind = kind;
    position = mul(view.view_projection, float4(world, 1.0));
    return out;
}

float coverage(float field) {
    float soft = max(fwidth(field), 1e-4);
    return clamp(0.5 - field / soft, 0.0, 1.0);
}

float petal(float2 c) {
    float y = c.y;
    float width = sqrt(max(1.0 - y * y, 0.0)) * (0.5 + 0.5 * (y + 1.0) * 0.5);
    float body = abs(c.x) - width;
    float notch = (0.8 + abs(c.x) * 0.9) - y;
    return coverage(max(body, -notch));
}

float sparkle(float2 c) {
    float2 a = abs(c);
    float rays = exp(-a.x * 14.0) * exp(-a.y * 2.2) + exp(-a.y * 14.0) * exp(-a.x * 2.2);
    float core = exp(-dot(c, c) * 18.0);
    return clamp(rays * 0.8 + core, 0.0, 1.0) * (1.0 - smoothstep(0.85, 1.0, length(c)));
}

[shader("fragment")]
float4 fragment_main(ParticleVaryings varyings, float4 frag_coord: SV_Position, bool front_facing: SV_IsFrontFace)
    : SV_Target
{
    RikkaFrameRef frame = rikka_push.frame;
    RikkaSpace space = frame.look.space;
    RikkaScene scene = frame.look.scene;
    float2 c = varyings.corner;
    float r = length(c);
    float shape;
    float3 tint = space.particle_color;
    if (varyings.kind == RIKKA_PARTICLE_PETALS) {
        shape = petal(c);
        float base = smoothstep(-0.9, 0.5, c.y);
        tint = lerp(lerp(float3(1.0), space.particle_color, 0.25), space.particle_color, base);
        tint *= front_facing ? 1.0 : 0.85;
    } else if (varyings.kind == RIKKA_PARTICLE_SPARKLES) {
        shape = sparkle(c);
    } else if (varyings.kind == RIKKA_PARTICLE_SNOW) {
        shape = smoothstep(1.0, 0.45, r);
    } else {
        shape = max(exp(-r * r * 4.0) - exp(-4.0), 0.0) / (1.0 - exp(-4.0));
    }
    if (shape <= 0.002) {
        discard;
    }

    float soft = 1.0;
    float depth = rikka_read_depth(scene.depth, int2(frag_coord.xy));
    if (depth > 0.0) {
        float behind = space_distance_of(frame.view.projection, depth) - varyings.distance;
        soft = clamp(behind / max(varyings.size * 3.0, 0.02), 0.0, 1.0);
    }
    float near = frame.view.near;
    soft *= smoothstep(near * 2.0, near * 2.0 + 0.25, varyings.distance);

    float alpha = shape * varyings.opacity * soft;
    float3 radiance = tint * varyings.light + varyings.glow;
    return float4(radiance * alpha, alpha * ADDITIVE[varyings.kind]);
}
