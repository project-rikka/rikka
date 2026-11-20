float2 oct_wrap(float2 v) {
    return (1.0 - abs(v.yx)) * float2(v.x >= 0.0 ? 1.0 : -1.0, v.y >= 0.0 ? 1.0 : -1.0);
}

float2 octahedral(float3 n) {
    float sum = abs(n.x) + abs(n.y) + abs(n.z);
    if (!(sum > 1.0e-20)) {
        return float2(0.0);
    }
    float3 p = n / sum;
    return p.z >= 0.0 ? p.xy : oct_wrap(p.xy);
}

uint pack_direction(float3 n) {
    return packSnorm2x16(octahedral(n));
}

float3 unpack_direction(uint packed) {
    float2 e = unpackSnorm2x16ToFloat(packed);
    float3 n = float3(e, 1.0 - abs(e.x) - abs(e.y));
    if (n.z < 0.0) {
        n.xy = oct_wrap(n.xy);
    }
    return normalize(n);
}

float3 unpack_octahedral(float2 e) {
    float3 n = float3(e, 1.0 - abs(e.x) - abs(e.y));
    if (n.z < 0.0) {
        n.xy = oct_wrap(n.xy);
    }
    float length_squared = dot(n, n);
    return length_squared > 0.0 ? n * rsqrt(length_squared) : float3(0.0);
}
