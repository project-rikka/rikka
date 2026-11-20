import fluid;

struct Push {
    Settings settings;
    ReadVec4s sorted;
    ReadUints perm;
    ReadUints starts;
    ReadUvec2s record_velocities;
    Vec4s particles;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

struct Spread : INeighbourVisitor {
    uint i;
    float3 xi;
    float h2;
    float inv_h;
    float weights;
    float3 sum;
    float3 diagonal;
    float3 off;
    float neighbours;

    [mutating]
    void visit(uint j) {
        float4 other = pc.sorted[j];
        if (j == i || other.w < 0.0) {
            return;
        }
        float3 d = other.xyz - xi;
        float r2 = dot(d, d);
        if (r2 >= h2) {
            return;
        }
        float q = sqrt(r2) * inv_h;
        float w = 1.0 - q * q * q;
        weights += w;
        sum += d * w;
        diagonal += d * d * w;
        off += d.xxy * d.yzz * w;
        neighbours += 1.0;
    }
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    float4 own = pc.sorted[i];
    uint o = pc.perm[i];
    if (own.w < 0.0) {
        for (uint k = 0u; k < 4u; ++k) {
            pc.particles[4u * o + k] = float4(0.0);
        }
        return;
    }
    Settings s = pc.settings;
    float h2 = kernel_h2(s);
    float inv_h = kernel_inv_h(s);
    float3 xi = own.xyz;
    Spread spread_sum = { i, xi, h2, inv_h, 0.0, float3(0.0), float3(0.0), float3(0.0), 0.0 };
    for_neighbours(s, pc.starts, xi, spread_sum);
    float weights = spread_sum.weights;
    float3 sum = spread_sum.sum;
    float3 diagonal = spread_sum.diagonal;
    float3 off = spread_sum.off;
    float neighbours = spread_sum.neighbours;
    float3 centre = xi;
    float4 rotation = float4(0.0, 0.0, 0.0, 1.0);
    float3 radii = float3(s.look.w);
    if (neighbours >= s.look_limits.z && weights > 0.0) {
        float3 shift = sum / weights;
        float3 mean = xi + shift;
        centre = lerp(xi, mean, s.look.x * min(1.0, weights / s.look_limits.w));
        float3 spread_diagonal = diagonal / weights - shift * shift;
        float3 spread_off = off / weights - shift.xxy * shift.yzz;
        float3x3 spread = float3x3(
            spread_diagonal.x, spread_off.x, spread_off.y,
            spread_off.x, spread_diagonal.y, spread_off.z,
            spread_off.y, spread_off.z, spread_diagonal.z);
        float3x3 axes;
        eigen_decompose(spread, axes);
        float3 sigma = sqrt(max(float3(spread[0][0], spread[1][1], spread[2][2]), float3(0.0))) * s.look.y;
        radii = s.look.z * clamp(sigma, float3(s.look_limits.x), float3(s.look_limits.y));
        if (determinant(axes) < 0.0) {
            axes[2] = -axes[2];
        }
        rotation = quat_from_basis(axes);
    }
    uint2 packed = pc.record_velocities[o];
    float3 velocity = float3(unpackHalf2x16ToFloat(packed.x), unpackHalf2x16ToFloat(packed.y).x);
    pc.particles[4u * o] = float4(centre, asfloat(uint(own.w)));
    pc.particles[4u * o + 1u] = rotation;
    pc.particles[4u * o + 2u] = float4(radii, 0.0);
    pc.particles[4u * o + 3u] = float4(velocity, 0.0);
}
