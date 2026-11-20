import fluid.settings;
import fluid.walls;

static const float VISCOUS_CONVERGED = 1.0e-20;

struct Coupling {
    float k;
    float3 d;
};

Coupling viscous_coupling(Settings settings, ReadUvec2s normals, ReadUints phases, ReadVec4s positions, float3 xi, float rho_i, float nu_i, uint j, float dt) {
    Coupling coupling = Coupling(0.0, float3(0.0));
    float nu = mixed(settings) ? 0.5 * (nu_i + settings.substances[substance_of(phases[j])].flow.x) : nu_i;
    coupling.d = xi - positions[j].xyz;
    float r2 = dot(coupling.d, coupling.d);
    if (nu > 0.0 && r2 < kernel_h2(settings) && r2 > 0.0) {
        float r = sqrt(r2);
        coupling.k = 10.0 * nu * dt * (-kernel_dw(r, kernel_inv_h(settings))) * kernel_inv_s0(settings)
            / (rho_i * normal_of(normals, j).w * r * (r2 + 0.01 * kernel_h2(settings)));
    }
    return coupling;
}

float boundary_coupling(Settings s, ReadVec4s contacts, uint i, float nu, float dt, out float3 surface) {
    float4 contact = contacts[i];
    surface = contact.xyz;
    return contact.w * nu * dt * s.boundary.x;
}

void store_symmetric(Floats diagonal, uint stride, uint i, float3x3 m) {
    diagonal[i] = m[0][0];
    diagonal[stride + i] = m[0][1];
    diagonal[2u * stride + i] = m[0][2];
    diagonal[3u * stride + i] = m[1][1];
    diagonal[4u * stride + i] = m[1][2];
    diagonal[5u * stride + i] = m[2][2];
}

float3 apply_symmetric(ReadFloats diagonal, uint stride, uint i, float3 v) {
    float d00 = diagonal[i];
    float d01 = diagonal[stride + i];
    float d02 = diagonal[2u * stride + i];
    float d11 = diagonal[3u * stride + i];
    float d12 = diagonal[4u * stride + i];
    float d22 = diagonal[5u * stride + i];
    return float3(
        d00 * v.x + d01 * v.y + d02 * v.z,
        d01 * v.x + d11 * v.y + d12 * v.z,
        d02 * v.x + d12 * v.y + d22 * v.z);
}
