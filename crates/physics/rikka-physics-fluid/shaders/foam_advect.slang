import fluid;

struct Push {
    Settings settings;
    ReadVec4s positions;
    ReadVec4s velocities;
    ReadUints phases;
    ReadUints starts;
    Vec4s foam_positions;
    Vec4s foam_velocities;
    Uints foam_meta;
    Uints alive;
    ReadUints counters;
    uint resident;
    float dt;
    float gx;
    float gy;
    float gz;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

struct Flow : INeighbourVisitor {
    float3 x;
    float h2;
    float inv_h;
    float weights;
    float3 flow;
    float neighbours;

    [mutating]
    void visit(uint j) {
        if (is_dead(pc.phases[j])) {
            return;
        }
        float3 d = x - pc.positions[j].xyz;
        float r2 = dot(d, d);
        if (r2 >= h2) {
            return;
        }
        float w = kernel_w(sqrt(r2), inv_h);
        weights += w;
        flow += pc.velocities[j].xyz * w;
        neighbours += 1.0;
    }
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i > pc.resident) {
        return;
    }
    if (i >= pc.counters[FOAM_COUNT]) {
        pc.alive[i] = 0u;
        return;
    }
    Settings s = pc.settings;
    float4 own = pc.foam_positions[i];
    float3 x = own.xyz;
    float life = own.w;
    float3 v = pc.foam_velocities[i].xyz;
    uint meta = pc.foam_meta[i];
    float h2 = kernel_h2(s);
    float inv_h = kernel_inv_h(s);
    Flow sampled = { x, h2, inv_h, 0.0, float3(0.0), 0.0 };
    for_neighbours(s, pc.starts, x, sampled);
    float weights = sampled.weights;
    float3 flow = sampled.flow;
    float neighbours = sampled.neighbours;
    float3 average = weights > 0.0 ? flow / weights : v;
    float3 gravity = float3(pc.gx, pc.gy, pc.gz);
    uint kind = neighbours < s.foam_kinds.x ? FOAM_SPRAY : (neighbours > s.foam_kinds.y ? FOAM_BUBBLE : FOAM_FOAM);
    float lifetime = s.foam_motion.x;
    if (kind == FOAM_SPRAY) {
        v = (v + gravity * pc.dt) / (1.0 + s.foam_motion.y * pc.dt);
        life -= pc.dt / (2.0 * lifetime);
    } else if (kind == FOAM_FOAM) {
        v = average;
        life -= pc.dt / lifetime;
    } else {
        v = v - s.foam_motion.z * gravity * pc.dt + min(s.foam_motion.w * pc.dt, 1.0) * (average - v);
        life -= pc.dt / (4.0 * lifetime);
    }
    x += v * pc.dt;
    if (s.body.w > 0.5 && x.y < s.body.z) {
        x.y = s.body.z;
        v.y = max(v.y, 0.0);
        v.xz *= 0.5;
    }
    bool outside = outside_bounds(s, x);
    bool lives = life > 0.0 && !outside;
    pc.foam_positions[i] = float4(x, life);
    pc.foam_velocities[i] = float4(v, 0.0);
    pc.foam_meta[i] = (meta & ~0xffu) | kind;
    pc.alive[i] = lives ? 1u : 0u;
}
