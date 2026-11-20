import physics.memory;
import physics.scan;

struct Push {
    Uints sums;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(1024, 1, 1)]
void main(uint3 local : SV_GroupThreadID) {
    scan_quad(pc.sums, local.x * 4u, pc.count);
}
