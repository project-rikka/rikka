import fluid;
import physics.triangle;

struct Push {
    Settings settings;
    Vec4s foam_positions;
    Vec4s foam_velocities;
    ReadUints alive;
    ReadVec4s skinned;
    ReadUvec4s triangles;
    ReadUints tri_starts;
    ReadUints tri_items;
    uint resident;
    float dt;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint LONGEST_MARCH = 48u;
static const float BOUNCE = 0.2;
static const float GRIP = 0.6;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.resident || pc.alive[i] == 0u) {
        return;
    }
    Settings s = pc.settings;
    float4 own = pc.foam_positions[i];
    float3 x = own.xyz;
    float3 v = pc.foam_velocities[i].xyz;
    float3 start = x - v * pc.dt;
    float3 path = x - start;
    float thickness = s.body.x + 0.25 * s.bounds_lo.w;
    uint samples = min(uint(ceil(2.0 * length(path) * s.tri_cell.y)) + 1u, LONGEST_MARCH);
    float best = 2.0;
    uint best_t = NONE;
    float3 normal = float3(0.0);
    float3 landing = x;
    uint previous_bin = NONE;
    for (uint k = 0u; k < samples; ++k) {
        float along = samples > 1u ? float(k) / float(samples - 1u) : 1.0;
        uint bin = bin_of(cell_of(start + path * along, s.tri_cell.y), s.tri_grid);
        if (bin == previous_bin) {
            continue;
        }
        previous_bin = bin;
        uint first = pc.tri_starts[bin];
        uint end = pc.tri_starts[bin + 1u];
        for (uint e = first; e < end; ++e) {
            uint t = pc.tri_items[e];
            uint4 tri = pc.triangles[t];
            float3 a = pc.skinned[tri.x].xyz;
            float3 b = pc.skinned[tri.y].xyz;
            float3 c = pc.skinned[tri.z].xyz;
            float3 across = cross(b - a, c - a);
            float area = length(across);
            if (area <= 0.0) {
                continue;
            }
            float3 n = across / area;
            float before = dot(start - a, n);
            float after = dot(x - a, n);
            float3 side = before >= 0.0 ? n : -n;
            if ((before >= 0.0) != (after >= 0.0)) {
                float crossing = before / (before - after);
                if (crossing > best || (crossing == best && t >= best_t)) {
                    continue;
                }
                float3 hit = start + path * crossing;
                bool face = closest_on_triangle(hit, a, b, c).face;
                if (face) {
                    best = crossing;
                    best_t = t;
                    normal = side;
                    landing = hit + side * thickness;
                }
                continue;
            }
            if (abs(after) >= thickness || best < 1.0 || (best == 1.0 && t >= best_t)) {
                continue;
            }
            bool face = closest_on_triangle(x, a, b, c).face;
            if (face) {
                best = 1.0;
                best_t = t;
                normal = side;
                landing = x + side * (thickness - abs(after));
            }
        }
    }
    if (best > 1.0) {
        return;
    }
    float into = dot(v, normal);
    if (into < 0.0) {
        float3 tangential = v - normal * into;
        v = tangential * GRIP - normal * (into * BOUNCE);
    }
    pc.foam_positions[i] = float4(landing, own.w);
    pc.foam_velocities[i] = float4(v, 0.0);
}
