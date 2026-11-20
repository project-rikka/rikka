import plane;
import color;

struct Push {
    Head head;
    float* data;
    uint cube_size;
    uint premultiply;
    int2 from;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint MATRIX = 0u;
static const uint CUBE = 1u;
static const uint MATRIX_AT = 6u * TABLE_SIZE;
static const uint CUBE_AT = MATRIX_AT + 12u;

float3 cube_at(int3 at) {
    uint n = pc.cube_size;
    uint index = CUBE_AT + 3u * ((uint(at.b) * n + uint(at.g)) * n + uint(at.r));
    return float3(pc.data[index], pc.data[index + 1u], pc.data[index + 2u]);
}

float3 through_cube(float3 c) {
    float last = float(pc.cube_size - 1u);
    float3 position = clamp(c, 0.0, 1.0) * last;
    int3 low = int3(min(floor(position), float3(last - 1.0)));
    float3 t = position - float3(low);
    float3 c000 = cube_at(low);
    float3 c100 = cube_at(low + int3(1, 0, 0));
    float3 c010 = cube_at(low + int3(0, 1, 0));
    float3 c110 = cube_at(low + int3(1, 1, 0));
    float3 c001 = cube_at(low + int3(0, 0, 1));
    float3 c101 = cube_at(low + int3(1, 0, 1));
    float3 c011 = cube_at(low + int3(0, 1, 1));
    float3 c111 = cube_at(low + int3(1, 1, 1));
    float3 x00 = lerp(c000, c100, t.r);
    float3 x10 = lerp(c010, c110, t.r);
    float3 x01 = lerp(c001, c101, t.r);
    float3 x11 = lerp(c011, c111, t.r);
    return lerp(lerp(x00, x10, t.g), lerp(x01, x11, t.g), t.b);
}

float3 through_matrix(float3 c) {
    float3 linear = float3(
        table_sample(pc.data, 0u, c.r),
        table_sample(pc.data, TABLE_SIZE, c.g),
        table_sample(pc.data, 2u * TABLE_SIZE, c.b));
    float3 mixed;
    for (int row = 0; row < 3; row++) {
        uint at = MATRIX_AT + uint(row) * 4u;
        mixed[row] = dot(float3(pc.data[at], pc.data[at + 1u], pc.data[at + 2u]), linear);
    }
    float3 root = sqrt(clamp(mixed, 0.0, 1.0));
    return float3(
        table_sample(pc.data, 3u * TABLE_SIZE, root.r),
        table_sample(pc.data, 4u * TABLE_SIZE, root.g),
        table_sample(pc.data, 5u * TABLE_SIZE, root.b));
}

[shader("compute")]
[numthreads(16, 16, 1)]
void convert(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 c = pc.head.src.load(pixel + pc.from);
    if (pc.head.mode == MATRIX) {
        c.rgb = through_matrix(c.rgb);
    } else if (pc.head.mode == CUBE) {
        c.rgb = through_cube(c.rgb);
    }
    c = pc.head.clamped(c);
    if (pc.premultiply != 0u) {
        c = clamp(c, 0.0, 1.0);
        c.rgb *= c.a;
    }
    pc.head.dst.store(pixel, c);
}
