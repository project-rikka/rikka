import fluid;

struct Push {
    Settings settings;
    Vec4s positions;
    ReadVec4s predicted;
    Uints phases;
    Vec4s velocities;
    Vec4s motion;
    Vec4s contacts;
    uint count;
    float dt;
    float max_speed;
    uint shown;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint phase = pc.phases[i];
    Settings s = pc.settings;
    bool grounded = s.body.w > 0.5;
    if (is_dead(phase)) {
        if (grounded) {
            pc.contacts[i] = float4(0.0);
        }
        return;
    }
    float4 start = pc.positions[i];
    float3 x0 = start.xyz;
    float3 x = pc.predicted[i].xyz;
    if (grounded) {
        x = floored(s, x0, x, phase);
        float ground = s.body.z + s.bounds_lo.w;
        float falloff = 1.0 - clamp((x.y - ground) / s.body.y, 0.0, 1.0);
        float drag = s.substances[substance_of(phase)].contact.y;
        x = dragged(x0, x, float3(0.0, 1.0, 0.0), float3(0.0), drag * falloff * pc.dt);
        pc.contacts[i] = float4(0.0, 0.0, 0.0, falloff);
    }
    float3 v = (x - x0) / pc.dt;
    float speed = length(v);
    if (speed > pc.max_speed) {
        v *= pc.max_speed / speed;
        x = x0 + v * pc.dt;
    }
    float life = pc.velocities[i].w - pc.dt;
    bool outside = outside_bounds(s, x);
    pc.phases[i] = (outside || life <= 0.0) ? (phase | DEAD) : (phase & ~FRESH);
    pc.positions[i] = float4(x, start.w);
    pc.velocities[i] = float4(v, life);
    if (pc.shown != 0u) {
        pc.motion[i] = float4(v, 0.0);
    }
}
