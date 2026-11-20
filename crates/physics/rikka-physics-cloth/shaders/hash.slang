import cloth.common;

struct Push {
    Uints source;
    Uints hashes;
    uint words;
    uint at;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(256, 1, 1)]
void main(uint3 local : SV_GroupThreadID) {
    uint lane = local.x;
    uint h = 2166136261u;
    for (uint w = lane; w < pc.words; w += 256u) {
        h = (h ^ pc.source[w]) * 16777619u;
        h ^= w;
    }
    pc.hashes[pc.at * 256u + lane] = h;
}
