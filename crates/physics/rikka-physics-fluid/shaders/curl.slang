import fluid;

struct Push {
    Settings settings;
    ReadVec4s positions;
    ReadVec4s velocities_in;
    Vec4s velocities_out;
    ReadUints phases;
    ReadUints list;
    ReadUints listed;
    Floats vorticity_magnitude;
    ReadUvec2s normals;
    Uints foam_counts;
    uint count;
    float dt;
    uint potentials;
    uint seed;
    float gx;
    float gy;
    float gz;
    uint stride;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

float ramp(float value, float low, float high) {
    return (min(value, high) - min(value, low)) / (high - low);
}

struct CurlWalk : IListedVisitor {
    Settings s;
    float3 xi;
    float3 vi;
    float swirl_i;
    float h2;
    float inv_h;
    uint substance;
    bool whirls;
    bool potentials;
    float normal_length;
    float3 outward;
    float3 smoothing;
    float3 vorticity;
    float3 rising;
    float trapped;
    float crest;

    [mutating]
    void visit(uint slot, uint j) {
        float4 other = pc.positions[j];
        float3 d = xi - other.xyz;
        float r2 = dot(d, d);
        if (r2 >= h2 || r2 <= 0.0) {
            return;
        }
        float r = sqrt(r2);
        float3 e = d / r;
        float3 vj = pc.velocities_in[j].xyz;
        float w = kernel_w(r, inv_h);
        float3 gradient = kernel_dw(r, inv_h) * e;
        vorticity += cross(gradient, vj - vi);
        if (kind_of(s, pc.phases, j, substance) == substance) {
            smoothing += (vj - vi) * w;
            if (whirls) {
                rising += (other.w - swirl_i) * gradient;
            }
        }
        if (potentials) {
            float4 normal_j = normal_of(pc.normals, j);
            float3 vij = vi - vj;
            float relative = length(vij);
            if (relative > 0.0) {
                trapped += relative * (1.0 - dot(vij / relative, e)) * w;
            }
            float other_length = length(normal_j.xyz);
            if (normal_length > 0.0 && other_length > 0.0 && dot(d, outward) > 0.0) {
                crest += (1.0 - dot(outward, -normal_j.xyz / other_length)) * w;
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
    if (i == 0u && pc.potentials != 0u) {
        pc.foam_counts[pc.count] = 0u;
    }
    float4 own = pc.velocities_in[i];
    uint phase = pc.phases[i];
    if (is_dead(phase)) {
        pc.velocities_out[i] = own;
        pc.vorticity_magnitude[i] = 0.0;
        if (pc.potentials != 0u) {
            pc.foam_counts[i] = 0u;
        }
        return;
    }
    Settings s = pc.settings;
    float h2 = kernel_h2(s);
    float inv_h = kernel_inv_h(s);
    float inv_s0 = kernel_inv_s0(s);
    uint substance = substance_of(phase);
    Substance kind = s.substances[substance];
    float4 own_position = pc.positions[i];
    float3 xi = own_position.xyz;
    float3 vi = own.xyz;
    bool potentials = pc.potentials != 0u;
    float normal_length = 0.0;
    float3 outward = float3(0.0);
    if (potentials) {
        float4 normal_i = normal_of(pc.normals, i);
        normal_length = length(normal_i.xyz);
        outward = normal_length > 0.0 ? -normal_i.xyz / normal_length : float3(0.0);
    }
    CurlWalk walk = {
        s, xi, vi, own_position.w, h2, inv_h, substance, kind.flow.z > 0.0, potentials,
        normal_length, outward, float3(0.0), float3(0.0), float3(0.0), 0.0, 0.0
    };
    for_listed(pc.list, pc.listed, pc.stride, i, walk);
    float3 smoothing = walk.smoothing;
    float3 vorticity = walk.vorticity;
    float3 rising = walk.rising;
    float trapped = walk.trapped;
    float crest = walk.crest;
    float3 fall = float3(pc.gx, pc.gy, pc.gz) * pc.dt;
    vorticity *= inv_s0;
    float length_rising = length(rising);
    float3 confined = length_rising > 0.0 ? kind.flow.z * cross(rising / length_rising, vorticity) : float3(0.0);
    pc.velocities_out[i] = float4(vi + smoothing * (kind.flow.y * inv_s0) + fall + confined * pc.dt, own.w);
    pc.vorticity_magnitude[i] = length(vorticity);
    if (potentials) {
        float speed = length(vi);
        float facing = speed > 0.0 ? dot(vi / speed, outward) : 0.0;
        float crest_potential = facing >= 0.6 ? ramp(crest, s.foam_potential.z, s.foam_potential.w) : 0.0;
        float air_potential = ramp(trapped, s.foam_potential.x, s.foam_potential.y);
        float energy = ramp(0.5 * speed * speed, s.foam_energy.x, s.foam_energy.y);
        float expected = energy * (s.foam_energy.z * air_potential + s.foam_energy.w * crest_potential) * pc.dt * kind.flow.w;
        float whole = floor(expected);
        float spare = expected - whole;
        uint extra = unit_random(i * 0x9e3779b9u + pc.seed) < spare ? 1u : 0u;
        pc.foam_counts[i] = min(uint(whole) + extra, 8u);
    }
}
