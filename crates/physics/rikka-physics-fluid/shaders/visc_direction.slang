import fluid;
import fluid.reduction;

struct Push {
    ReadUints phases;
    ReadVec4s preconditioned;
    Vec4s direction;
    Sums sums;
    uint count;
    uint now;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    float rz_now = reduced(pc.sums, pc.now);
    if (rz_now <= VISCOUS_CONVERGED) {
        return;
    }
    if (is_dead(pc.phases[i])) {
        pc.direction[i] = float4(0.0);
        return;
    }
    float3 z = pc.preconditioned[i].xyz;
    float rz_before = reduced(pc.sums, 1u - pc.now);
    float beta = rz_before > 0.0 ? rz_now / rz_before : 0.0;
    pc.direction[i] = float4(z + beta * pc.direction[i].xyz, 0.0);
}
