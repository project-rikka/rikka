__exported import rikka.common;

static const uint RIKKA_EMITTER_POINT = 0u;
static const uint RIKKA_EMITTER_SPHERE = 1u;
static const uint RIKKA_EMITTER_BOX = 2u;
static const uint RIKKA_EMITTER_DISC = 3u;
static const uint RIKKA_EMITTER_CONE = 4u;

static const uint RIKKA_GROUND_NONE = 0u;
static const uint RIKKA_GROUND_BOUNCE = 1u;
static const uint RIKKA_GROUND_DIE = 2u;

static const uint RIKKA_FACING_CAMERA = 0u;
static const uint RIKKA_FACING_VELOCITY = 1u;
static const uint RIKKA_FACING_HORIZONTAL = 2u;
static const uint RIKKA_FACING_AXIS = 3u;
static const uint RIKKA_FACING_TUMBLE = 4u;

static const uint RIKKA_PARTICLE_BLEND_ALPHA = 0u;
static const uint RIKKA_PARTICLE_BLEND_ADDITIVE = 1u;
static const uint RIKKA_PARTICLE_BLEND_PREMULTIPLIED = 2u;

static const uint RIKKA_EMITTER_MAX_BURSTS = 16u;
static const uint RIKKA_EMITTER_MAX_BOUNCES = 8u;
static const float RIKKA_EMITTER_REST_SPEED = 0.05;
static const uint RIKKA_EMITTER_NEWTON_STEPS = 12u;
static const float RIKKA_EMITTER_SERIES_LIMIT = 0.1;
static const float RIKKA_EMITTER_STEEP = -1e-6;
static const uint RIKKA_EMITTER_BURST_KEY = 0x80000000u;
static const float RIKKA_EMITTER_UNIT = 1.0 / 16777216.0;
static const uint RIKKA_EMITTER_STREAM_SALT = 0x632BE5ABu;
static const uint RIKKA_EMITTER_KEY_SPREAD = 0x9E3779B9u;
static const float3 RIKKA_EMITTER_SECOND_FIELD = float3(31.416, 47.853, 12.679);
static const float3 RIKKA_EMITTER_THIRD_FIELD = float3(73.156, 9.731, 58.367);

struct RikkaEmitterBurst {
    float time;
    float duration;
    uint count;
    uint pad;
};

struct RikkaEmitterKey {
    float3 translation;
    float time;
    float4 rotation;
    float3 scale;
    float pad;
};

struct RikkaEmitter {
    uint first_particle;
    uint slots;
    uint capacity;
    uint seed;
    uint first_key;
    uint key_count;
    uint burst_count;
    uint shape;
    float start;
    float end;
    float rate;
    float lifetime;
    float lifetime_variance;
    float speed;
    float speed_variance;
    float inherit;
    float3 launch;
    float radius;
    float3 extent;
    float angle;
    float3 gravity;
    float drag;
    float3 wind;
    float drag_variance;
    float3 noise_offset;
    float turbulence;
    float3 attractor;
    float attraction;
    float frequency;
    float flow;
    float attraction_rate;
    uint ground;
    float ground_height;
    float restitution;
    float friction;
    float ground_fade;
    float size_variance;
    float rotation;
    float spin;
    float spin_variance;
    float stretch;
    float stretch_min;
    float stretch_max;
    float softness;
    float twinkle;
    float twinkle_rate;
    float translucency;
    uint layers;
    float3 axis;
    uint facing;
    uint sprite;
    uint sampler;
    uint columns;
    uint rows;
    float frame_rate;
    uint random_frame;
    uint blend;
    uint lit;
    uint bloom;
    uint look;
    uint coloring;
    float aspect;
    float4 size_ages;
    float4 size_values;
    float4 opacity_ages;
    float4 opacity_values;
    float4 emission_ages;
    float4 emission_values;
    float4 color_ages;
    float4 colors[4];
    RikkaEmitterBurst bursts[16];
};

struct RikkaEmitterParticle {
    float3 position;
    float size;
    float3 previous;
    float opacity;
    float3 velocity;
    float rotation;
    uint4 shade;
};

typealias RikkaEmitterRef = RikkaEmitter*;
typealias RikkaEmitterKeyRef = RikkaEmitterKey*;
typealias RikkaEmitterParticleRef = RikkaEmitterParticle*;

struct RikkaEmitterStream {
    uint state;

    [mutating]
    float next() {
        state = rikka_pcg_hash(state);
        return float(state >> 8u) * RIKKA_EMITTER_UNIT;
    }

    uint hash() {
        return rikka_pcg_hash(state);
    }
};

RikkaEmitterStream rikka_emitter_stream(uint seed, uint key) {
    RikkaEmitterStream stream;
    stream.state = rikka_pcg_hash(key * RIKKA_EMITTER_KEY_SPREAD ^ rikka_pcg_hash(seed + RIKKA_EMITTER_STREAM_SALT));
    return stream;
}

float rikka_emitter_jitter(uint seed, uint key) {
    RikkaEmitterStream stream = rikka_emitter_stream(seed, key);
    return stream.next();
}

struct RikkaEmitterDraws {
    float life;
    float3 shape;
    float2 direction;
    float speed;
    float drag;
    float size;
    float rotation;
    float spin;
    float turn;
    float twinkle;
    float frame;
    float color;
    uint hash;
};

RikkaEmitterDraws rikka_emitter_draws(uint seed, uint key) {
    RikkaEmitterStream stream = rikka_emitter_stream(seed, key);
    stream.next();
    RikkaEmitterDraws draws;
    draws.life = stream.next();
    draws.shape.x = stream.next();
    draws.shape.y = stream.next();
    draws.shape.z = stream.next();
    draws.direction.x = stream.next();
    draws.direction.y = stream.next();
    draws.speed = stream.next();
    draws.drag = stream.next();
    draws.size = stream.next();
    draws.rotation = stream.next();
    draws.spin = stream.next();
    draws.turn = stream.next();
    draws.twinkle = stream.next();
    draws.frame = stream.next();
    draws.color = stream.next();
    draws.hash = stream.hash();
    return draws;
}

uint2 rikka_emitter_locate(float4 ages, float age) {
    if (age <= ages[0]) {
        return uint2(0u, asuint(0.0));
    }
    for (uint at = 0u; at < 3u; ++at) {
        if (age <= ages[at + 1u]) {
            float span = max(ages[at + 1u] - ages[at], 1e-6);
            return uint2(at, asuint(clamp((age - ages[at]) / span, 0.0, 1.0)));
        }
    }
    return uint2(3u, asuint(0.0));
}

float rikka_emitter_curve(float4 ages, float4 values, float age) {
    uint2 found = rikka_emitter_locate(ages, age);
    uint to = min(found.x + 1u, 3u);
    return values[found.x] + (values[to] - values[found.x]) * asfloat(found.y);
}

float3 rikka_emitter_gradient(RikkaEmitterRef emitter, float age) {
    uint2 found = rikka_emitter_locate(emitter.color_ages, age);
    uint to = min(found.x + 1u, 3u);
    float3 from = emitter.colors[found.x].rgb;
    return from + (emitter.colors[to].rgb - from) * asfloat(found.y);
}

float3 rikka_emitter_rotate(float4 rotation, float3 v) {
    float3 axis = rotation.xyz;
    float3 t = cross(axis, v) * 2.0;
    return v + t * rotation.w + cross(axis, t);
}

struct RikkaEmitterPlacement {
    float3 translation;
    float4 rotation;
    float3 scale;
    float3 velocity;
};

RikkaEmitterPlacement rikka_emitter_placement(RikkaEmitterRef emitter, RikkaEmitterKeyRef keys, float time) {
    uint count = emitter.key_count;
    RikkaEmitterKey first = keys[emitter.first_key];
    RikkaEmitterKey last = keys[emitter.first_key + count - 1u];
    RikkaEmitterPlacement placement;
    placement.velocity = float3(0.0);
    if (count == 1u || time <= first.time) {
        placement.translation = first.translation;
        placement.rotation = first.rotation;
        placement.scale = first.scale;
        return placement;
    }
    if (time >= last.time) {
        placement.translation = last.translation;
        placement.rotation = last.rotation;
        placement.scale = last.scale;
        return placement;
    }
    uint at = 0u;
    while (at + 2u < count && time >= keys[emitter.first_key + at + 1u].time) {
        at += 1u;
    }
    RikkaEmitterKey from = keys[emitter.first_key + at];
    RikkaEmitterKey to = keys[emitter.first_key + at + 1u];
    float span = to.time - from.time;
    float mix = (time - from.time) / span;
    float4 rotation = from.rotation + (to.rotation - from.rotation) * mix;
    placement.translation = from.translation + (to.translation - from.translation) * mix;
    placement.rotation = rotation / length(rotation);
    placement.scale = from.scale + (to.scale - from.scale) * mix;
    placement.velocity = (to.translation - from.translation) / span;
    return placement;
}

float3 rikka_emitter_sphere(float2 d) {
    float y = 1.0 - 2.0 * d.x;
    float reach = sqrt(max(1.0 - y * y, 0.0));
    float phi = RIKKA_TAU * d.y;
    return float3(reach * cos(phi), y, reach * sin(phi));
}

void rikka_emitter_shape(RikkaEmitterRef emitter, float3 s, float2 d, out float3 local, out float3 direction) {
    float3 up = float3(0.0, 1.0, 0.0);
    float turn = RIKKA_TAU * s.x;
    float reach = emitter.radius * sqrt(s.y);
    float3 disc = float3(reach * cos(turn), 0.0, reach * sin(turn));
    switch (emitter.shape) {
    case RIKKA_EMITTER_POINT:
        local = float3(0.0);
        direction = rikka_emitter_sphere(d);
        break;
    case RIKKA_EMITTER_SPHERE:
        direction = rikka_emitter_sphere(d);
        local = direction * (emitter.radius * pow(s.x, 1.0 / 3.0));
        break;
    case RIKKA_EMITTER_BOX:
        local = (2.0 * s - 1.0) * emitter.extent;
        direction = up;
        break;
    case RIKKA_EMITTER_DISC:
        local = disc;
        direction = up;
        break;
    case RIKKA_EMITTER_CONE: {
        float cos_theta = 1.0 - d.x * (1.0 - cos(emitter.angle));
        float sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
        float phi = RIKKA_TAU * d.y;
        local = disc;
        direction = float3(sin_theta * cos(phi), cos_theta, sin_theta * sin(phi));
        break;
    }
    default:
        local = float3((s.x - 0.5) * emitter.radius, 0.0, 0.0);
        direction = up;
        break;
    }
}

float rikka_emitter_phi(float x) {
    if (x < RIKKA_EMITTER_SERIES_LIMIT) {
        return 1.0 + x * (-0.5 + x * (1.0 / 6.0 + x * (-1.0 / 24.0 + x * (1.0 / 120.0))));
    }
    return (1.0 - exp(-x)) / x;
}

float rikka_emitter_psi(float x) {
    if (x < RIKKA_EMITTER_SERIES_LIMIT) {
        return 0.5 + x * (-1.0 / 6.0 + x * (1.0 / 24.0 + x * (-1.0 / 120.0 + x * (1.0 / 720.0))));
    }
    return (1.0 - rikka_emitter_phi(x)) / x;
}

void rikka_emitter_ballistic(float3 x, float3 v, float3 gravity, float3 wind, float drag, float t,
                             out float3 position, out float3 velocity) {
    float k = drag * t;
    float travel = t * rikka_emitter_phi(k);
    float fall = t * t * rikka_emitter_psi(k);
    position = x + wind * t + (v - wind) * travel + gravity * fall;
    velocity = wind + (v - wind) * exp(-k) + gravity * travel;
}

float rikka_emitter_height(float height, float vy, float gy, float wy, float drag, float t) {
    float k = drag * t;
    return height + wy * t + (vy - wy) * (t * rikka_emitter_phi(k)) + gy * (t * t * rikka_emitter_psi(k));
}

float rikka_emitter_rise(float vy, float gy, float wy, float drag, float t) {
    float k = drag * t;
    return wy + (vy - wy) * exp(-k) + gy * (t * rikka_emitter_phi(k));
}

float rikka_emitter_descend(float height, float vy, float gy, float wy, float drag, float from, float limit) {
    float t = from;
    for (uint step = 0u; step < RIKKA_EMITTER_NEWTON_STEPS; ++step) {
        float slope = rikka_emitter_rise(vy, gy, wy, drag, t);
        if (slope >= RIKKA_EMITTER_STEEP) {
            break;
        }
        t = clamp(t - rikka_emitter_height(height, vy, gy, wy, drag, t) / slope, 0.0, limit);
    }
    return t;
}

float rikka_emitter_contact(float height, float vy, float gy, float wy, float drag, float span) {
    if (height < 0.0 || (height == 0.0 && vy <= 0.0)) {
        return 0.0;
    }
    if (!(span > 0.0)) {
        return -1.0;
    }
    float bend = gy + drag * (wy - vy);
    if (bend <= 0.0) {
        if (rikka_emitter_height(height, vy, gy, wy, drag, span) > 0.0) {
            return -1.0;
        }
        return rikka_emitter_descend(height, vy, gy, wy, drag, span, span);
    }
    if (vy >= 0.0) {
        return -1.0;
    }
    float lowest = span;
    if (rikka_emitter_rise(vy, gy, wy, drag, span) > 0.0) {
        float t = 0.0;
        for (uint step = 0u; step < RIKKA_EMITTER_NEWTON_STEPS; ++step) {
            float slope = bend * exp(-drag * t);
            t = clamp(t - rikka_emitter_rise(vy, gy, wy, drag, t) / slope, 0.0, span);
        }
        lowest = t;
    }
    if (rikka_emitter_height(height, vy, gy, wy, drag, lowest) > 0.0) {
        return -1.0;
    }
    return rikka_emitter_descend(height, vy, gy, wy, drag, 0.0, lowest);
}

struct RikkaEmitterFlight {
    float3 position;
    float3 velocity;
    float rest;
    bool alive;
};

RikkaEmitterFlight rikka_emitter_fly(RikkaEmitterRef emitter, float3 origin, float3 start, float drag, float age) {
    RikkaEmitterFlight flight;
    flight.rest = asfloat(0x7f800000u);
    flight.alive = true;
    if (emitter.ground == RIKKA_GROUND_NONE) {
        rikka_emitter_ballistic(origin, start, emitter.gravity, emitter.wind, drag, age,
            flight.position, flight.velocity);
        return flight;
    }
    float3 x = origin;
    float3 v = start;
    float begun = 0.0;
    float keep = 1.0 - clamp(emitter.friction, 0.0, 1.0);
    for (uint bounce = 0u; bounce <= RIKKA_EMITTER_MAX_BOUNCES; ++bounce) {
        float span = age - begun;
        float hit = rikka_emitter_contact(x.y - emitter.ground_height, v.y, emitter.gravity.y,
            emitter.wind.y, drag, span);
        if (hit < 0.0) {
            rikka_emitter_ballistic(x, v, emitter.gravity, emitter.wind, drag, span,
                flight.position, flight.velocity);
            return flight;
        }
        if (emitter.ground == RIKKA_GROUND_DIE) {
            flight.alive = false;
            flight.position = x;
            flight.velocity = float3(0.0);
            return flight;
        }
        float3 position;
        float3 velocity;
        rikka_emitter_ballistic(x, v, emitter.gravity, emitter.wind, drag, hit, position, velocity);
        position.y = emitter.ground_height;
        float up = -emitter.restitution * velocity.y;
        if (up < RIKKA_EMITTER_REST_SPEED || bounce == RIKKA_EMITTER_MAX_BOUNCES) {
            flight.position = position;
            flight.velocity = float3(0.0);
            flight.rest = begun + hit;
            return flight;
        }
        x = position;
        v = float3(velocity.x * keep, up, velocity.z * keep);
        begun += hit;
    }
    flight.position = x;
    flight.velocity = float3(0.0);
    return flight;
}

static const float3 RIKKA_EMITTER_GRADIENTS[12] = {
    float3(1.0, 1.0, 0.0), float3(-1.0, 1.0, 0.0), float3(1.0, -1.0, 0.0), float3(-1.0, -1.0, 0.0),
    float3(1.0, 0.0, 1.0), float3(-1.0, 0.0, 1.0), float3(1.0, 0.0, -1.0), float3(-1.0, 0.0, -1.0),
    float3(0.0, 1.0, 1.0), float3(0.0, -1.0, 1.0), float3(0.0, 1.0, -1.0), float3(0.0, -1.0, -1.0) };

float3 rikka_emitter_corner(int3 cell) {
    uint hash = rikka_pcg_hash(uint(cell.x) ^ rikka_pcg_hash(uint(cell.y) ^ rikka_pcg_hash(uint(cell.z))));
    return RIKKA_EMITTER_GRADIENTS[hash % 12u];
}

float3 rikka_emitter_noise_gradient(float3 p) {
    float3 lower = floor(p);
    int3 cell = int3(lower);
    float3 f = p - lower;
    float3 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    float3 du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
    float3 ga = rikka_emitter_corner(cell);
    float3 gb = rikka_emitter_corner(cell + int3(1, 0, 0));
    float3 gc = rikka_emitter_corner(cell + int3(0, 1, 0));
    float3 gd = rikka_emitter_corner(cell + int3(1, 1, 0));
    float3 ge = rikka_emitter_corner(cell + int3(0, 0, 1));
    float3 gf = rikka_emitter_corner(cell + int3(1, 0, 1));
    float3 gg = rikka_emitter_corner(cell + int3(0, 1, 1));
    float3 gh = rikka_emitter_corner(cell + int3(1, 1, 1));
    float va = dot(ga, f);
    float vb = dot(gb, f - float3(1.0, 0.0, 0.0));
    float vc = dot(gc, f - float3(0.0, 1.0, 0.0));
    float vd = dot(gd, f - float3(1.0, 1.0, 0.0));
    float ve = dot(ge, f - float3(0.0, 0.0, 1.0));
    float vf = dot(gf, f - float3(1.0, 0.0, 1.0));
    float vg = dot(gg, f - float3(0.0, 1.0, 1.0));
    float vh = dot(gh, f - float3(1.0, 1.0, 1.0));
    float k1 = vb - va;
    float k2 = vc - va;
    float k3 = ve - va;
    float k4 = va - vb - vc + vd;
    float k5 = va - vc - ve + vg;
    float k6 = va - vb - ve + vf;
    float k7 = -va + vb + vc - vd + ve - vf - vg + vh;
    float3 blended = ga
        + u.x * (gb - ga)
        + u.y * (gc - ga)
        + u.z * (ge - ga)
        + u.x * u.y * (ga - gb - gc + gd)
        + u.y * u.z * (ga - gc - ge + gg)
        + u.z * u.x * (ga - gb - ge + gf)
        + u.x * u.y * u.z * (-ga + gb + gc - gd + ge - gf - gg + gh);
    float3 slope = float3(
        k1 + k4 * u.y + k6 * u.z + k7 * u.y * u.z,
        k2 + k5 * u.z + k4 * u.x + k7 * u.z * u.x,
        k3 + k6 * u.x + k5 * u.y + k7 * u.x * u.y);
    return blended + du * slope;
}

float3 rikka_emitter_curl(float3 p) {
    float3 a = rikka_emitter_noise_gradient(p);
    float3 b = rikka_emitter_noise_gradient(p + RIKKA_EMITTER_SECOND_FIELD);
    float3 c = rikka_emitter_noise_gradient(p + RIKKA_EMITTER_THIRD_FIELD);
    return float3(c.y - b.z, a.z - c.x, b.x - a.y);
}

struct RikkaEmitterBirth {
    uint key;
    float age;
    bool born;
};

bool rikka_emitter_latest(uint seed, uint slot, uint slots, float emitted, out uint key) {
    key = 0u;
    float ahead = emitted - float(slot);
    if (!(ahead >= 0.0)) {
        return false;
    }
    uint cycle = uint(floor(ahead / float(slots)));
    key = cycle * slots + slot;
    if (float(key) + rikka_emitter_jitter(seed, key) <= emitted) {
        return true;
    }
    if (cycle == 0u) {
        return false;
    }
    key -= slots;
    return true;
}

RikkaEmitterBirth rikka_emitter_birth(RikkaEmitterRef emitter, uint index, float time) {
    RikkaEmitterBirth birth;
    birth.key = 0u;
    birth.age = 0.0;
    birth.born = false;
    if (index < emitter.slots) {
        float rate = emitter.rate;
        float emitted = (time - emitter.start) * rate;
        uint key;
        if (!rikka_emitter_latest(emitter.seed, index, emitter.slots, emitted, key)) {
            return birth;
        }
        if (!isinf(emitter.end)) {
            uint last;
            if (!rikka_emitter_latest(emitter.seed, index, emitter.slots,
                    (emitter.end - emitter.start) * rate, last)) {
                return birth;
            }
            key = min(key, last);
        }
        float jitter = rikka_emitter_jitter(emitter.seed, key);
        birth.key = key;
        birth.age = (emitted - float(key) - jitter) / rate;
        birth.born = true;
        return birth;
    }
    uint rest = index - emitter.slots;
    for (uint at = 0u; at < emitter.burst_count; ++at) {
        RikkaEmitterBurst burst = emitter.bursts[at];
        if (rest < burst.count) {
            uint key = RIKKA_EMITTER_BURST_KEY | (at << 24u) | rest;
            float jitter = rikka_emitter_jitter(emitter.seed, key);
            float born = burst.time + burst.duration * (float(rest) + jitter) / float(burst.count);
            birth.key = key;
            birth.age = time - born;
            birth.born = true;
            return birth;
        }
        rest -= burst.count;
    }
    return birth;
}

struct RikkaParticleState {
    float3 position;
    float3 velocity;
    float age;
    float lifetime;
    float size;
    float3 color;
    float opacity;
    float emission;
    float rotation;
    uint frame;
    uint hash;
    bool alive;
};

struct RikkaParticleMotion {
    float3 origin;
    float3 start;
    float drag;
};

RikkaParticleMotion rikka_emitter_launch(RikkaEmitterRef emitter, RikkaEmitterKeyRef keys, RikkaEmitterDraws draws,
                                         float birth) {
    RikkaEmitterPlacement placement = rikka_emitter_placement(emitter, keys, birth);
    float3 local;
    float3 direction;
    rikka_emitter_shape(emitter, draws.shape, draws.direction, local, direction);
    float3 heading = rikka_emitter_rotate(placement.rotation, direction);
    float speed = emitter.speed * (1.0 + emitter.speed_variance * (2.0 * draws.speed - 1.0));
    RikkaParticleMotion motion;
    motion.origin = placement.translation + rikka_emitter_rotate(placement.rotation, placement.scale * local);
    motion.start = heading * speed + emitter.launch + placement.velocity * emitter.inherit;
    motion.drag = max(emitter.drag * (1.0 + emitter.drag_variance * (2.0 * draws.drag - 1.0)), 0.0);
    return motion;
}

RikkaEmitterFlight rikka_emitter_path(RikkaEmitterRef emitter, RikkaParticleMotion motion, float age) {
    RikkaEmitterFlight flight = rikka_emitter_fly(emitter, motion.origin, motion.start, motion.drag, age);
    if (emitter.turbulence != 0.0) {
        float active = min(age, flight.rest);
        float3 sample = motion.origin * emitter.frequency + emitter.noise_offset
            + float3(0.0, active * emitter.flow, 0.0);
        float3 swirl = rikka_emitter_curl(sample);
        flight.position += swirl * (emitter.turbulence * active);
        if (age < flight.rest) {
            flight.velocity += swirl * emitter.turbulence;
        }
        if (emitter.ground != RIKKA_GROUND_NONE) {
            flight.position.y = max(flight.position.y, emitter.ground_height);
        }
    }
    if (emitter.attraction != 0.0) {
        float fall = exp(-emitter.attraction_rate * age);
        float pull = emitter.attraction * (1.0 - fall);
        float3 toward = emitter.attractor - flight.position;
        flight.velocity = flight.velocity * (1.0 - pull)
            + toward * (emitter.attraction * emitter.attraction_rate * fall);
        flight.position += toward * pull;
    }
    return flight;
}

RikkaParticleState rikka_emitter_particle(RikkaEmitterRef emitter, RikkaEmitterKeyRef keys, uint index, float time,
                                          float previous_time, out float3 previous) {
    RikkaParticleState state = {};
    previous = float3(0.0);
    state.alive = false;
    RikkaEmitterBirth birth = rikka_emitter_birth(emitter, index, time);
    if (!birth.born) {
        return state;
    }
    float age = birth.age;
    RikkaEmitterDraws draws = rikka_emitter_draws(emitter.seed, birth.key);
    float lifetime = emitter.lifetime * (1.0 - clamp(emitter.lifetime_variance, 0.0, 1.0) * draws.life);
    if (!(age >= 0.0 && age <= lifetime)) {
        return state;
    }
    RikkaParticleMotion motion = rikka_emitter_launch(emitter, keys, draws, time - age);
    RikkaEmitterFlight flight = rikka_emitter_path(emitter, motion, age);
    if (!flight.alive) {
        return state;
    }
    float before = age - (time - previous_time);
    if (previous_time == time) {
        previous = flight.position;
    } else if (before < 0.0) {
        previous = flight.position - flight.velocity * (time - previous_time);
    } else {
        previous = rikka_emitter_path(emitter, motion, before).position;
    }

    float normalized = clamp(age / max(lifetime, 1e-6), 0.0, 1.0);
    float wave = 0.5 + 0.5 * sin(RIKKA_TAU * (emitter.twinkle_rate * age + draws.twinkle));
    float blink = wave * wave;
    float twinkle = 1.0 - emitter.twinkle * (1.0 - blink * blink);
    float fade = 1.0;
    if (emitter.ground != RIKKA_GROUND_NONE && emitter.ground_fade > 0.0 && flight.rest <= age) {
        fade = clamp(1.0 - (age - flight.rest) / emitter.ground_fade, 0.0, 1.0);
    }
    float spin = emitter.spin * (1.0 + emitter.spin_variance * (2.0 * draws.spin - 1.0))
        * (draws.turn < 0.5 ? -1.0 : 1.0);
    uint frame = 0u;
    if (emitter.sprite != RIKKA_NONE) {
        uint frames = max(emitter.columns, 1u) * max(emitter.rows, 1u);
        uint first = emitter.random_frame != 0u ? uint(draws.frame * float(frames)) : 0u;
        uint step = emitter.frame_rate > 0.0 ? uint(age * emitter.frame_rate)
            : min(uint(normalized * float(frames)), frames - 1u);
        frame = (first + step) % frames;
    }
    state.position = flight.position;
    state.velocity = flight.velocity;
    state.age = age;
    state.lifetime = lifetime;
    state.size = rikka_emitter_curve(emitter.size_ages, emitter.size_values, normalized)
        * (1.0 + emitter.size_variance * (2.0 * draws.size - 1.0));
    state.color = rikka_emitter_gradient(emitter, emitter.coloring != 0u ? draws.color : normalized);
    state.opacity = rikka_emitter_curve(emitter.opacity_ages, emitter.opacity_values, normalized) * twinkle * fade;
    state.emission = rikka_emitter_curve(emitter.emission_ages, emitter.emission_values, normalized) * twinkle;
    state.rotation = emitter.rotation * (2.0 * draws.rotation - 1.0) + spin * age;
    state.frame = frame;
    state.hash = draws.hash;
    state.alive = true;
    return state;
}
