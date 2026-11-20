import fluid.settings;
import physics.octahedral;

static const uint WALLS_NONE = 0u;
static const uint WALLS_FLOOR = 1u;
static const uint WALLS_STORED = 2u;

uint2 pack_wall(float4 wall, float3 x) {
    uint packed = pack_direction(wall.xyz);
    float3 normal = unpack_direction(packed);
    return uint2(packed, asuint(dot(normal, x) - wall.w));
}

float4 wall_at(ReadUvec2s walls, uint i, float3 x) {
    uint2 packed = walls[i];
    float3 normal = unpack_direction(packed.x);
    return float4(normal, dot(normal, x) - asfloat(packed.y));
}

float4 wall_for(Settings s, uint walled, ReadUvec2s walls, uint i, float3 x) {
    if (walled == WALLS_STORED) {
        return wall_at(walls, i, x);
    }
    if (walled == WALLS_FLOOR) {
        return float4(0.0, 1.0, 0.0, x.y - s.body.z - s.bounds_lo.w + s.body.x);
    }
    return float4(0.0, 1.0, 0.0, FOREVER);
}

uint2 pack_normal(float4 normal) {
    return uint2(packHalf2x16(normal.xy), packHalf2x16(normal.zw));
}

float4 normal_of(ReadUvec2s normals, uint i) {
    uint2 packed = normals[i];
    return float4(unpackHalf2x16ToFloat(packed.x), unpackHalf2x16ToFloat(packed.y));
}
