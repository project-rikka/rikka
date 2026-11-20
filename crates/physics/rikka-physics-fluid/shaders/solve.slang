import fluid;

struct Push {
    Settings settings;
    ReadVec4s predicted;
    Vec4s predicted_out;
    ReadUints phases;
    ReadUints list;
    ReadUints listed;
    ReadUvec2s normals;
    ReadUvec2s walls;
    Floats clearances;
    uint count;
    uint first;
    float dt;
    uint stride;
    uint walled;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const float COHESIVE_FADE = 10.0;

struct SolveWalk : IListedVisitor {
    Settings s;
    float3 xi;
    float lambda_i;
    float h2;
    float inv_h;
    uint substance;
    bool cohesive;
    float cohesion;
    float rho_i;
    float3 pressure;
    float3 attraction;

    [mutating]
    void visit(uint slot, uint j) {
        float4 charge = pc.predicted[j];
        float3 d = xi - charge.xyz;
        float r2 = dot(d, d);
        if (r2 >= h2 || r2 <= 0.0) {
            return;
        }
        float r = sqrt(r2);
        float3 e = d / r;
        pressure += (lambda_i + charge.w) * kernel_dw(r, inv_h) * e;
        if (cohesive && kind_of(s, pc.phases, j, substance) == substance) {
            float rho_j = normal_of(pc.normals, j).w;
            float sparse = clamp((1.0 - min(rho_i, rho_j)) * COHESIVE_FADE, 0.0, 1.0);
            attraction -= (2.0 * sparse / (rho_i + rho_j)) * cohesion * cohesion_shape(r, inv_h) * e;
        }
    }
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    float4 own = pc.predicted[i];
    uint phase = pc.phases[i];
    if (is_dead(phase)) {
        pc.predicted_out[i] = float4(own.xyz, 0.0);
        return;
    }
    Settings s = pc.settings;
    float h2 = kernel_h2(s);
    float inv_h = kernel_inv_h(s);
    float inv_s0 = kernel_inv_s0(s);
    uint substance = substance_of(phase);
    Substance kind = s.substances[substance];
    bool cohesive = pc.first != 0u && kind.stick.x > 0.0;
    float3 xi = own.xyz;
    float lambda_i = own.w;
    float rho_i = cohesive ? normal_of(pc.normals, i).w : 1.0;
    SolveWalk walk = { s, xi, lambda_i, h2, inv_h, substance, cohesive, kind.stick.x, rho_i, float3(0.0), float3(0.0) };
    for_listed(pc.list, pc.listed, pc.stride, i, walk);
    float3 pressure = walk.pressure;
    float3 attraction = walk.attraction;
    float4 wall = float4(0.0, 1.0, 0.0, FOREVER);
    if (pc.walled != WALLS_NONE) {
        wall = wall_for(s, pc.walled, pc.walls, i, xi);
        if (wall.w < s.body.x + kernel_h(s)) {
            pressure += 2.0 * lambda_i * wall_density(s, wall.w).y * wall.xyz;
        }
    }
    float dt2 = pc.dt * pc.dt;
    float3 delta = pressure * (inv_s0 * s.solver.y) + attraction * (dt2 / (1.0 + kind.contact.z * dt2));
    float reach = s.bounds_lo.w;
    float moved = length(delta);
    if (moved > reach) {
        delta *= reach / moved;
    }
    delta += wall.xyz * max(s.body.x - (wall.w + dot(delta, wall.xyz)), 0.0);
    if (pc.walled == WALLS_STORED) {
        pc.clearances[i] -= length(delta);
    }
    pc.predicted_out[i] = float4(xi + delta, 0.0);
}
