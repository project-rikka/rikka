import fluid;

struct Push {
    Settings settings;
    Vec4s predicted;
    ReadUints phases;
    ReadUints list;
    ReadUints listed;
    Uvec2s normals;
    ReadUvec2s walls;
    ReadFloats dilations;
    uint count;
    uint stride;
    uint walled;
    uint shaped;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;
static const uint SHAPED_NONE = 0u;
static const uint SHAPED_DENSITY = 1u;
static const uint SHAPED_NORMAL = 2u;

struct DensityWalk : IListedVisitor {
    float3 xi;
    float h2;
    float inv_h;
    uint substance;
    bool track_normal;
    float density;
    float3 gradient;
    float squares;
    float3 normal;

    [mutating]
    void visit(uint slot, uint j) {
        float3 d = xi - pc.predicted[j].xyz;
        float r2 = dot(d, d);
        if (r2 >= h2) {
            return;
        }
        float r = sqrt(r2);
        density += kernel_w(r, inv_h);
        if (r > 0.0) {
            float dw = kernel_dw(r, inv_h);
            float3 g = (dw / r) * d;
            gradient += g;
            squares += dw * dw;
            if (track_normal && substance_of(pc.phases[j]) == substance) {
                normal += g;
            }
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
    uint phase = pc.phases[i];
    if (is_dead(phase)) {
        pc.predicted.at(i)->w = 0.0;
        if (pc.shaped != SHAPED_NONE) {
            pc.normals[i] = pack_normal(float4(0.0, 0.0, 0.0, 1.0));
        }
        return;
    }
    Settings s = pc.settings;
    float h2 = kernel_h2(s);
    float inv_h = kernel_inv_h(s);
    float inv_s0 = kernel_inv_s0(s);
    uint substance = substance_of(phase);
    float3 xi = pc.predicted[i].xyz;
    bool is_mixed = mixed(s);
    DensityWalk walk = { xi, h2, inv_h, substance, pc.shaped == SHAPED_NORMAL && is_mixed, 0.0, float3(0.0), 0.0, float3(0.0) };
    for_listed(pc.list, pc.listed, pc.stride, i, walk);
    float density = walk.density;
    float3 gradient = walk.gradient;
    float squares = walk.squares;
    float3 surface_normal = is_mixed ? walk.normal : gradient;
    if (pc.walled != WALLS_NONE) {
        float4 wall = wall_for(s, pc.walled, pc.walls, i, xi);
        if (wall.w < s.body.x + kernel_h(s)) {
            float2 ghost = wall_density(s, wall.w);
            density += ghost.x;
            gradient += ghost.y * wall.xyz;
        }
    }
    float relative = density * inv_s0;
    float constraint = max(relative - 1.0, 0.0);
    if (relative < 1.0 && s.substances[substance].bond.x > 0.0) {
        float steepness = length(gradient) * inv_s0 * kernel_h(s);
        constraint = closing_constraint(s, relative, steepness, pc.dilations[i]);
    }
    float denominator = (dot(gradient, gradient) + squares) * inv_s0 * inv_s0 + s.solver.x;
    pc.predicted.at(i)->w = -constraint / denominator;
    if (pc.shaped != SHAPED_NONE) {
        float3 packed_normal = pc.shaped == SHAPED_NORMAL ? surface_normal * (kernel_h(s) * inv_s0) : float3(0.0);
        pc.normals[i] = pack_normal(float4(packed_normal, max(relative, 0.25)));
    }
}
