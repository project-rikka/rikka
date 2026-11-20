import rikka.emitter.push;
import rikka.shadow;
import rikka.sky;

static const float FORWARD_SCATTER = 0.5;
static const float HALF_LIMIT = 65000.0;

float scatter_phase(float cosine) {
    float g2 = FORWARD_SCATTER * FORWARD_SCATTER;
    float denominator = 1.0 + g2 - 2.0 * FORWARD_SCATTER * cosine;
    return (1.0 - g2) / (4.0 * RIKKA_PI * denominator * sqrt(denominator));
}

float3 incident(RikkaFrameRef frame, float3 position, float translucency) {
    RikkaViewRef view = frame.view;
    float3 eye = normalize(position - view.camera_position);
    float view_depth = dot(position - view.camera_position, view.camera_forward);
    float3 direct = float3(0.0);
    for (uint i = 0u; i < view.directional_count; ++i) {
        RikkaLight light = frame.lights[i];
        if (light.figure != 0u) {
            continue;
        }
        float3 toward = -light.direction;
        float shadow = 1.0;
        if (light.shadow != RIKKA_NONE && (light.shadow & RIKKA_SHADOW_UNMAPPED) == 0u) {
            uint cascade = rikka_cascade(frame, light, view_depth);
            shadow = rikka_shadow_lookup(frame, frame.shadows[light.shadow + cascade], position, toward);
        }
        float facing = 0.5 + 0.5 * dot(-eye, toward);
        float response = (1.0 - translucency) * facing * RIKKA_INV_PI
            + translucency * scatter_phase(dot(eye, toward));
        direct += light.color * light.intensity * shadow * response;
    }
    float3 ambient = 0.5 * (rikka_sky_irradiance(frame, float3(0.0, 1.0, 0.0)) + rikka_sky_irradiance(frame, -eye));
    return direct + ambient;
}

uint pack_halves(float low, float high) {
    return f32tof16(clamp(low, 0.0, HALF_LIMIT)) | (f32tof16(clamp(high, 0.0, HALF_LIMIT)) << 16u);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void compute_main(uint3 id: SV_DispatchThreadID) {
    RikkaEmitterRef emitter = rikka_pushed_emitter();
    uint index = id.x;
    if (index >= emitter.capacity) {
        return;
    }
    RikkaFrameRef frame = rikka_emitter_push.frame;
    float time = frame.view.time;
    float previous_time = frame.look.scene.previous_time;
    float3 previous;
    RikkaParticleState state = rikka_emitter_particle(emitter, rikka_emitter_push.keys, index, time,
        previous_time, previous);
    RikkaEmitterParticle particle = {};
    if (state.alive && state.size > 0.0 && state.opacity > 0.0) {
        float3 lit = float3(0.0);
        if (emitter.lit != 0u) {
            lit = state.color * incident(frame, state.position, clamp(emitter.translucency, 0.0, 1.0));
        }
        float3 emitted = state.color * max(state.emission, 0.0);
        particle.position = state.position;
        particle.size = state.size;
        particle.previous = previous;
        particle.opacity = min(state.opacity, 1.0);
        particle.velocity = state.velocity;
        particle.rotation = state.rotation;
        particle.shade = uint4(
            pack_halves(lit.r, lit.g),
            pack_halves(lit.b, emitted.r),
            pack_halves(emitted.g, emitted.b),
            (state.frame << 16u) | (state.hash & 0xffffu));
    }
    rikka_emitter_push.particles[emitter.first_particle + index] = particle;
}
