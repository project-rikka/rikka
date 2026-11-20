import cloth.common;
import cloth.world;

struct Push {
    Figures figures;
    Uints cells;
    Uints items;
    Vec4s spheres;
    Uints large;
    uint listed;
    uint placing;
    float epoch_span;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

void insert(uint id, float3 low, float3 high) {
    float radius = 0.5 * length(high - low);
    float4 sphere = float4(0.5 * (low + high), (id & CLOTH) != 0u ? -radius : radius);
    uint level = LEVELS;
    for (uint candidate = 0u; candidate < LEVELS && level == LEVELS; ++candidate) {
        if (all(grid_cell(high, candidate) - grid_cell(low, candidate) <= int3(1))) {
            level = candidate;
        }
    }
    if (level == LEVELS) {
        if (pc.placing == 0u) {
            uint slot = queue_atomic_add(pc.large.at(0), 1u);
            if (slot < LARGE_OBSTACLES) {
                pc.large[1u + slot] = id;
            }
        }
        return;
    }
    int3 from = grid_cell(low, level);
    int3 to = grid_cell(high, level);
    for (int z = from.z; z <= to.z; ++z) {
        for (int y = from.y; y <= to.y; ++y) {
            for (int x = from.x; x <= to.x; ++x) {
                uint cell = grid_hash(int3(x, y, z), level);
                if (pc.placing == 0u) {
                    queue_atomic_add(pc.cells.at(cell), 1u);
                } else {
                    uint slot = queue_atomic_add(pc.cells.at(GRID_CELLS + 1u + cell), 1u);
                    pc.items[slot] = id;
                    pc.spheres[slot] = sphere;
                }
            }
        }
    }
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint at = thread.x;
    uint figure = 0u;
    for (; figure < pc.listed; ++figure) {
        uint4 counts = pc.figures[figure].counts;
        uint obstacles = counts.w + counts.y;
        if (at < obstacles) {
            break;
        }
        at -= obstacles;
    }
    if (figure == pc.listed) {
        return;
    }
    Figure f = pc.figures[figure];
    uint body_triangles = f.counts.w;
    uint id = figure << FIGURE_SHIFT;
    if (at < body_triangles) {
        uint4 corners = f.body_triangles[at];
        float3 a0 = f.body_from[corners.x].xyz;
        float3 b0 = f.body_from[corners.y].xyz;
        float3 c0 = f.body_from[corners.z].xyz;
        float3 a1 = f.body_to[corners.x].xyz;
        float3 b1 = f.body_to[corners.y].xyz;
        float3 c1 = f.body_to[corners.z].xyz;
        float3 low0 = min(min(a0, b0), c0);
        float3 high0 = max(max(a0, b0), c0);
        float3 low1 = min(min(a1, b1), c1);
        float3 high1 = max(max(a1, b1), c1);
        insert(id | at, min(low0, low1), max(high0, high1));
        return;
    }
    uint t = at - body_triangles;
    uint4 corners = f.triangles[t];
    float3 a0 = f.epoch_start[corners.x].xyz;
    float3 b0 = f.epoch_start[corners.y].xyz;
    float3 c0 = f.epoch_start[corners.z].xyz;
    float3 a1 = a0 + clamp_length(f.epoch_motion[corners.x].xyz * pc.epoch_span, MAX_SWEEP_STEP);
    float3 b1 = b0 + clamp_length(f.epoch_motion[corners.y].xyz * pc.epoch_span, MAX_SWEEP_STEP);
    float3 c1 = c0 + clamp_length(f.epoch_motion[corners.z].xyz * pc.epoch_span, MAX_SWEEP_STEP);
    float3 low = min(min(min(a0, b0), c0), min(min(a1, b1), c1));
    float3 high = max(max(max(a0, b0), c0), max(max(a1, b1), c1));
    float cover = max(max(f.rest[corners.x].w, f.rest[corners.y].w), f.rest[corners.z].w);
    if (cover > 0.0) {
        float3 lift0 = face_normal(a0, b0, c0) * cover;
        float3 lift1 = face_normal(a1, b1, c1) * cover;
        low += min(min(lift0, lift1), float3(0.0));
        high += max(max(lift0, lift1), float3(0.0));
    }
    insert(id | CLOTH | t, low, high);
}
