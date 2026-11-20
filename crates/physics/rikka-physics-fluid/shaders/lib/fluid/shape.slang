float4 quat_from_basis(float3x3 m) {
    float trace = m[0][0] + m[1][1] + m[2][2];
    float4 q;
    if (trace > 0.0) {
        float s = sqrt(trace + 1.0) * 2.0;
        q = float4((m[1][2] - m[2][1]) / s, (m[2][0] - m[0][2]) / s, (m[0][1] - m[1][0]) / s, 0.25 * s);
    } else if (m[0][0] > m[1][1] && m[0][0] > m[2][2]) {
        float s = sqrt(1.0 + m[0][0] - m[1][1] - m[2][2]) * 2.0;
        q = float4(0.25 * s, (m[1][0] + m[0][1]) / s, (m[2][0] + m[0][2]) / s, (m[1][2] - m[2][1]) / s);
    } else if (m[1][1] > m[2][2]) {
        float s = sqrt(1.0 + m[1][1] - m[0][0] - m[2][2]) * 2.0;
        q = float4((m[1][0] + m[0][1]) / s, 0.25 * s, (m[2][1] + m[1][2]) / s, (m[2][0] - m[0][2]) / s);
    } else {
        float s = sqrt(1.0 + m[2][2] - m[0][0] - m[1][1]) * 2.0;
        q = float4((m[2][0] + m[0][2]) / s, (m[2][1] + m[1][2]) / s, 0.25 * s, (m[0][1] - m[1][0]) / s);
    }
    return normalize(q);
}

void jacobi_rotate(inout float3x3 a, inout float3x3 r, int p, int q) {
    if (a[q][p] == 0.0) {
        return;
    }
    float d = (a[p][p] - a[q][q]) / (2.0 * a[q][p]);
    float t = 1.0 / (abs(d) + sqrt(d * d + 1.0));
    if (d < 0.0) {
        t = -t;
    }
    float c = 1.0 / sqrt(t * t + 1.0);
    float s = t * c;
    a[p][p] += t * a[q][p];
    a[q][q] -= t * a[q][p];
    a[q][p] = 0.0;
    a[p][q] = 0.0;
    for (int k = 0; k < 3; ++k) {
        if (k != p && k != q) {
            float akp = c * a[p][k] + s * a[q][k];
            float akq = -s * a[p][k] + c * a[q][k];
            a[p][k] = akp;
            a[k][p] = akp;
            a[q][k] = akq;
            a[k][q] = akq;
        }
    }
    for (int k = 0; k < 3; ++k) {
        float rkp = c * r[p][k] + s * r[q][k];
        float rkq = -s * r[p][k] + c * r[q][k];
        r[p][k] = rkp;
        r[q][k] = rkq;
    }
}

void eigen_decompose(inout float3x3 a, out float3x3 r) {
    r = float3x3(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    for (int sweep = 0; sweep < 6; ++sweep) {
        float x01 = abs(a[1][0]);
        float x02 = abs(a[2][0]);
        float x12 = abs(a[2][1]);
        if (max(x01, max(x02, x12)) < 1.0e-20) {
            return;
        }
        if (x01 >= x02 && x01 >= x12) {
            jacobi_rotate(a, r, 0, 1);
        } else if (x02 >= x12) {
            jacobi_rotate(a, r, 0, 2);
        } else {
            jacobi_rotate(a, r, 1, 2);
        }
    }
}
