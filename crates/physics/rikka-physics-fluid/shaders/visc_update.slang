import fluid;
import fluid.reduction;

struct Push {
    ReadVec4s direction;
    ReadVec4s product;
    ReadFloats diagonal;
    Vec4s iterate;
    Vec4s preconditioned;
    Sums sums;
    ReadVec4s targets;
    Vec4s results;
    Vec4s warm;
    ReadFloats vorticity_magnitude;
    Vec4s positions;
    uint count;
    uint stride;
    uint last;
    uint now;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID, uint3 local : SV_GroupThreadID, uint3 group : SV_GroupID) {
    uint i = thread.x;
    uint t = local.x;
    float rz = reduced(pc.sums, pc.now);
    bool converged = rz <= VISCOUS_CONVERGED;
    if (pc.last != 0u) {
        if (i < pc.count) {
            float3 x = pc.iterate[i].xyz;
            if (!converged) {
                float pq = reduced(pc.sums, 2u);
                float alpha = pq > 1.0e-30 ? rz / pq : 0.0;
                x += alpha * pc.direction[i].xyz;
            }
            float4 target = pc.targets[i];
            pc.results[i] = float4(x, target.w);
            pc.warm[i] = float4(x - target.xyz, 0.0);
            pc.positions.at(i)->w = pc.vorticity_magnitude[i];
        }
        return;
    }
    if (converged) {
        if (group.x == 0u && t == 0u) {
            pc.sums[1u - pc.now] = 0.0;
        }
        return;
    }
    float pq = reduced(pc.sums, 2u);
    float alpha = pq > 1.0e-30 ? rz / pq : 0.0;
    float next = 0.0;
    if (i < pc.count) {
        float3 x = pc.iterate[i].xyz + alpha * pc.direction[i].xyz;
        float3 r = pc.iterate[pc.stride + i].xyz - alpha * pc.product[i].xyz;
        float3 z = apply_symmetric(pc.diagonal, pc.stride, i, r);
        pc.iterate[i] = float4(x, 0.0);
        pc.iterate[pc.stride + i] = float4(r, 0.0);
        pc.preconditioned[i] = float4(z, 0.0);
        next = dot(r, z);
    }
    reduce_groups(pc.sums, 1u - pc.now, next, t);
}
