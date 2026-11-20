import physics.memory;
import physics.scan;

struct Push {
    Uints values;
    Uints sums;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(256, 1, 1)]
void main(uint3 local : SV_GroupThreadID, uint3 group : SV_GroupID) {
    uint t = local.x;
    scan_quad(pc.values, group.x * 1024u + t * 4u, pc.count);
    if (t == 0u) {
        pc.sums[group.x] = scan_total();
    }
}
