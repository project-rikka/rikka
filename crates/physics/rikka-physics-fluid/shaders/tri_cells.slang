import fluid;

struct Push {
    Settings settings;
    ReadVec4s skin_from;
    ReadVec4s skin_to;
    ReadUvec4s triangles;
    Uints cells;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint t = thread.x;
    if (t > pc.count) {
        return;
    }
    if (t == pc.count) {
        pc.cells[t] = 0u;
        return;
    }
    pc.cells[t] = slab_count(triangle_slab(pc.settings, pc.skin_from, pc.skin_to, pc.triangles[t]));
}
