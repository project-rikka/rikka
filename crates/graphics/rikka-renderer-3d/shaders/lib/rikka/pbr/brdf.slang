__exported import rikka.material;

static const float RIKKA_PBR_EPSILON = 1e-4;
static const float RIKKA_PBR_MIN_ALPHA = 2e-3;
static const float RIKKA_PBR_FILTER_VARIANCE = 0.25;
static const float RIKKA_PBR_FILTER_CLAMP = 0.18;
static const float RIKKA_PBR_FON_A = 0.5 - 2.0 / (3.0 * RIKKA_PI);
static const float RIKKA_PBR_FON_AVERAGE = 2.0 / 3.0 - 28.0 / (15.0 * RIKKA_PI);
static const float RIKKA_PBR_F82_MU = 1.0 / 7.0;
static const uint RIKKA_PBR_LUT_SIZE = 32u;
static const uint RIKKA_PBR_COAT = 0u;
static const uint RIKKA_PBR_FUZZ = 1u;
static const uint RIKKA_PBR_FILM = 2u;
static const uint RIKKA_PBR_SUBSURFACE = 3u;
static const uint RIKKA_PBR_TRANSMISSION = 4u;
static const uint RIKKA_PBR_ANISOTROPY = 5u;
static const uint RIKKA_PBR_SECOND_LOBE = 6u;

float rikka_pbr_sq(float x) {
    return x * x;
}

float3 rikka_pbr_sq(float3 x) {
    return x * x;
}

float rikka_pbr_pow5(float x) {
    float x2 = x * x;
    return x2 * x2 * x;
}

struct RikkaPbrBasis {
    float3 t;
    float3 b;
    float3 n;

    float3 local(float3 v) {
        return float3(dot(v, t), dot(v, b), dot(v, n));
    }
};

RikkaPbrBasis rikka_pbr_basis(float3 n, float3 tangent) {
    RikkaPbrBasis basis;
    basis.n = n;
    float3 t = tangent - n * dot(n, tangent);
    float length2 = dot(t, t);
    if (length2 < 1e-8) {
        float3 axis = abs(n.y) < 0.999 ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0);
        t = cross(axis, n);
        length2 = dot(t, t);
    }
    basis.t = t * rsqrt(length2);
    basis.b = cross(n, basis.t);
    return basis;
}

float2 rikka_pbr_alpha(float roughness, float anisotropy) {
    float r2 = rikka_pbr_sq(clamp(roughness, 0.0, 1.0));
    float a = clamp(anisotropy, 0.0, 1.0);
    float along = r2 * sqrt(2.0 / (1.0 + rikka_pbr_sq(1.0 - a)));
    return max(float2(along, (1.0 - a) * along), float2(RIKKA_PBR_MIN_ALPHA));
}

float rikka_pbr_filter_kernel(float3 normal) {
    float3 dx = ddx(normal);
    float3 dy = ddy(normal);
    float variance = RIKKA_PBR_FILTER_VARIANCE * (dot(dx, dx) + dot(dy, dy));
    return min(2.0 * variance, RIKKA_PBR_FILTER_CLAMP);
}

float2 rikka_pbr_filtered_alpha(float2 alpha, float kernel) {
    return sqrt(clamp(alpha * alpha + kernel, float2(0.0), float2(1.0)));
}

float rikka_pbr_ggx(float3 h, float2 alpha) {
    float2 slope = h.xy / alpha;
    float d = dot(slope, slope) + h.z * h.z;
    return 1.0 / max(RIKKA_PI * alpha.x * alpha.y * d * d, 1e-12);
}

float rikka_pbr_visibility(float3 v, float3 l, float2 alpha) {
    float lv = l.z * length(float3(alpha.x * v.x, alpha.y * v.y, v.z));
    float ll = v.z * length(float3(alpha.x * l.x, alpha.y * l.y, l.z));
    return 0.5 / max(lv + ll, 1e-7);
}

float rikka_pbr_microfacet(float3 v, float3 l, float2 alpha) {
    float3 h = normalize(v + l);
    return rikka_pbr_ggx(h, alpha) * rikka_pbr_visibility(v, l, alpha);
}

float rikka_pbr_f0(float eta) {
    return rikka_pbr_sq((eta - 1.0) / (eta + 1.0));
}

float rikka_pbr_f90(float f0) {
    return clamp(50.0 * f0, 0.0, 1.0);
}

float3 rikka_pbr_schlick(float3 f0, float3 f90, float cosine) {
    return f0 + (f90 - f0) * rikka_pbr_pow5(1.0 - clamp(cosine, 0.0, 1.0));
}

float rikka_pbr_schlick(float f0, float cosine) {
    return f0 + (1.0 - f0) * rikka_pbr_pow5(1.0 - clamp(cosine, 0.0, 1.0));
}

float3 rikka_pbr_f82(float3 f0, float3 tint, float cosine) {
    float mu = clamp(cosine, 0.0, 1.0);
    float3 bar = rikka_pbr_schlick(f0, float3(1.0), RIKKA_PBR_F82_MU);
    float3 schlick = rikka_pbr_schlick(f0, float3(1.0), mu);
    float scale = mu * pow(1.0 - mu, 6.0) / (RIKKA_PBR_F82_MU * pow(1.0 - RIKKA_PBR_F82_MU, 6.0));
    return clamp(schlick - scale * bar * (float3(1.0) - tint), float3(0.0), float3(1.0));
}

float rikka_pbr_modulated_eta(float eta, float weight) {
    float fresnel = rikka_pbr_f0(eta);
    float xi = clamp(weight, 0.0, 1.0 / max(fresnel, 1e-6));
    float epsilon = (eta >= 1.0 ? 1.0 : -1.0) * sqrt(xi * fresnel);
    return (1.0 + epsilon) / max(1.0 - epsilon, 1e-4);
}

float rikka_pbr_average_fresnel(float eta) {
    if (eta < 1.0) {
        float inverse = 1.0 / max(eta, 1e-3);
        float outer = log((10893.0 * inverse - 1438.2) / (-774.4 * inverse * inverse + 10212.0 * inverse + 1.0));
        return 1.0 - eta * eta * (1.0 - outer);
    }
    return clamp(log((10893.0 * eta - 1438.2) / (-774.4 * eta * eta + 10212.0 * eta + 1.0)), 0.0, 1.0);
}

float rikka_pbr_fon_albedo(float mu, float roughness) {
    float m = 1.0 - mu;
    float g = m * (0.0571085289 + m * (0.491881867 + m * (-0.332181442 + m * 0.0714429953)));
    return (1.0 + roughness * g) / (1.0 + RIKKA_PBR_FON_A * roughness);
}

float3 rikka_pbr_eon(float3 rho, float roughness, float3 wi, float3 wo) {
    if (roughness <= 0.0) {
        return rho / RIKKA_PI;
    }
    float mu_i = max(wi.z, RIKKA_PBR_EPSILON);
    float mu_o = max(wo.z, RIKKA_PBR_EPSILON);
    float s = dot(wi, wo) - mu_i * mu_o;
    float s_over_t = s > 0.0 ? s / max(mu_i, mu_o) : s;
    float a = 1.0 / (1.0 + RIKKA_PBR_FON_A * roughness);
    float3 single = rho * (a * (1.0 + roughness * s_over_t) / RIKKA_PI);
    float average = a * (1.0 + RIKKA_PBR_FON_AVERAGE * roughness);
    float3 multiple_rho = rho * rho * average / max(float3(1.0) - rho * (1.0 - average), float3(1e-4));
    float lost = max(1e-7, 1.0 - rikka_pbr_fon_albedo(mu_o, roughness))
        * max(1e-7, 1.0 - rikka_pbr_fon_albedo(mu_i, roughness)) / max(1e-7, 1.0 - average);
    return single + multiple_rho * (lost / RIKKA_PI);
}

float3 rikka_pbr_eon_albedo(float3 rho, float roughness, float mu) {
    if (roughness <= 0.0) {
        return rho;
    }
    float a = 1.0 / (1.0 + RIKKA_PBR_FON_A * roughness);
    float average = a * (1.0 + RIKKA_PBR_FON_AVERAGE * roughness);
    float albedo = rikka_pbr_fon_albedo(mu, roughness);
    float3 multiple_rho = rho * rho * average / max(float3(1.0) - rho * (1.0 - average), float3(1e-4));
    return rho * albedo + multiple_rho * (1.0 - albedo);
}

float rikka_pbr_sheen_albedo(float mu, float roughness) {
    float x = clamp(mu, 0.0, 1.0);
    float y = clamp(roughness, 0.01, 1.0);
    float s = y * (0.0206607 + 1.58491 * y) / (0.0379424 + y * (1.32227 + y));
    float m = y * (-0.193854 + y * (-1.14885 + y * (1.7932 - 0.95943 * y * y))) / (0.046391 + y);
    float o = y * (0.000654023 + (-0.0207818 + 0.119681 * y) * y) / (1.26264 + y * (-1.92021 + y));
    return clamp(exp(-0.5 * rikka_pbr_sq((x - m) / s)) / (s * sqrt(2.0 * RIKKA_PI)) + o, 0.0, 1.0);
}

float2 rikka_pbr_sheen_inverse(float n_dot_v, float roughness) {
    float y = clamp(roughness, 0.01, 1.0);
    float a_inv = (2.58126 * n_dot_v + 0.813703 * y) * y / (1.0 + 0.310327 * n_dot_v * n_dot_v + 2.60994 * n_dot_v * y);
    float b_inv = sqrt(max(1.0 - n_dot_v, 0.0)) * (y - 1.0) * y * y * y
        / (0.0000254053 + 1.71228 * n_dot_v - 1.71506 * n_dot_v * y + 1.34174 * y * y);
    return float2(a_inv, b_inv);
}

RikkaLtc rikka_pbr_sheen_ltc(float n_dot_v, float roughness) {
    float2 inverse = rikka_pbr_sheen_inverse(clamp(n_dot_v, 0.0, 1.0), roughness);
    RikkaLtc ltc;
    ltc.inverse = rikka_columns(float3(inverse.x, 0.0, 0.0), float3(0.0, inverse.x, 0.0), float3(inverse.y, 0.0, 1.0));
    ltc.magnitude = 1.0;
    ltc.fresnel = 0.0;
    return ltc;
}

float rikka_pbr_sheen_lobe(float3 n, float3 v, float3 l, float roughness) {
    float n_dot_v = clamp(dot(n, v), 0.0, 1.0);
    float3 x_axis = v - n * n_dot_v;
    float length2 = dot(x_axis, x_axis);
    x_axis = length2 > 1e-8 ? x_axis * rsqrt(length2) : (abs(n.y) < 0.999 ? normalize(cross(float3(0.0, 1.0, 0.0), n)) : float3(1.0, 0.0, 0.0));
    float3 y_axis = cross(n, x_axis);
    float3 local = float3(dot(l, x_axis), dot(l, y_axis), dot(l, n));
    float2 inverse = rikka_pbr_sheen_inverse(n_dot_v, roughness);
    float3 w = float3(inverse.x * local.x + inverse.y * local.z, inverse.x * local.y, local.z);
    float length_sq = dot(w, w);
    return max(w.z, 0.0) * inverse.x * inverse.x / (RIKKA_PI * length_sq * length_sq);
}

float3 rikka_pbr_film_sensitivity(float difference, float3 shift) {
    float phase = 2.0 * RIKKA_PI * difference * 1.0e-9;
    float3 value = float3(5.4856e-13, 4.4201e-13, 5.2481e-13);
    float3 position = float3(1.6810e+06, 1.7953e+06, 2.2084e+06);
    float3 spread = float3(4.3278e+09, 9.3046e+09, 6.6121e+09);
    float3 xyz = value * sqrt(2.0 * RIKKA_PI * spread) * cos(position * phase + shift) * exp(-phase * phase * spread);
    xyz.x += 9.7470e-14 * sqrt(2.0 * RIKKA_PI * 4.5282e+09) * cos(2.2399e+06 * phase + shift.x) * exp(-4.5282e+09 * phase * phase);
    xyz /= 1.0685e-7;
    return float3(
        3.2404542 * xyz.x - 1.5371385 * xyz.y - 0.4985314 * xyz.z,
        -0.9692660 * xyz.x + 1.8760108 * xyz.y + 0.0415560 * xyz.z,
        0.0556434 * xyz.x - 0.2040259 * xyz.y + 1.0572252 * xyz.z);
}

float3 rikka_pbr_film_ior(float3 f0) {
    float3 root = sqrt(clamp(f0, float3(0.0), float3(0.9999)));
    return (float3(1.0) + root) / (float3(1.0) - root);
}

float3 rikka_pbr_thin_film(float outside, float film_ior, float cosine, float thickness_nm, float3 base_f0) {
    float ior = lerp(outside, film_ior, smoothstep(0.0, 0.03, thickness_nm * 1e-3));
    float sin2 = rikka_pbr_sq(outside / ior) * (1.0 - cosine * cosine);
    float cos2_sq = 1.0 - sin2;
    if (cos2_sq < 0.0) {
        return float3(1.0);
    }
    float cos2 = sqrt(cos2_sq);
    float r0 = rikka_pbr_sq((ior - outside) / (ior + outside));
    float r12 = rikka_pbr_schlick(r0, cosine);
    float t121 = 1.0 - r12;
    float phi12 = ior < outside ? RIKKA_PI : 0.0;
    float phi21 = RIKKA_PI - phi12;
    float3 base_ior = rikka_pbr_film_ior(base_f0);
    float3 r1 = rikka_pbr_sq((base_ior - ior) / (base_ior + ior));
    float3 r23 = r1 + (float3(1.0) - r1) * rikka_pbr_pow5(1.0 - cos2);
    float3 phi23 = float3(base_ior.x < ior ? RIKKA_PI : 0.0, base_ior.y < ior ? RIKKA_PI : 0.0, base_ior.z < ior ? RIKKA_PI : 0.0);
    float difference = 2.0 * ior * thickness_nm * cos2;
    float3 phi = float3(phi21) + phi23;
    float3 r123 = clamp(r12 * r23, float3(1e-5), float3(0.9999));
    float3 root = sqrt(r123);
    float3 rs = t121 * t121 * r23 / (float3(1.0) - r123);
    float3 total = r12 + rs;
    float3 c = rs - t121;
    [unroll]
    for (int m = 1; m <= 2; ++m) {
        c *= root;
        total += c * 2.0 * rikka_pbr_film_sensitivity(float(m) * difference, float(m) * phi);
    }
    return max(total, float3(0.0));
}

float2 rikka_pbr_dfg(RikkaFrameRef frame, float n_dot_v, float roughness) {
    uint lut = frame.look.scene.brdf;
    if (lut == RIKKA_NONE) {
        return rikka_environment_brdf(n_dot_v, roughness);
    }
    float scale = float(RIKKA_PBR_LUT_SIZE - 1u) / float(RIKKA_PBR_LUT_SIZE);
    float bias = 0.5 / float(RIKKA_PBR_LUT_SIZE);
    float2 uv = float2(clamp(n_dot_v, 0.0, 1.0), clamp(roughness, 0.0, 1.0)) * scale + bias;
    return rikka_textures[NonUniformResourceIndex(lut)].SampleLevel(uv, 0.0).rg;
}

float3 rikka_pbr_energy(float3 f0, float2 dfg) {
    return float3(1.0) + f0 * (1.0 / max(dfg.x + dfg.y, 1e-3) - 1.0);
}

float3 rikka_pbr_specular_albedo(float3 f0, float3 f90, float2 dfg) {
    return f0 * dfg.x + f90 * dfg.y;
}

float rikka_pbr_horizon(float3 reflected, float3 geometric_normal) {
    float horizon = clamp(1.0 + 1.1 * dot(reflected, geometric_normal), 0.0, 1.0);
    return horizon * horizon;
}

float3 rikka_pbr_dominant(float3 n, float3 v, float roughness) {
    float3 reflected = reflect(-v, n);
    float a2 = rikka_pbr_sq(rikka_pbr_sq(roughness));
    return normalize(lerp(reflected, n, a2));
}

float3 rikka_pbr_bent_reflection(RikkaPbrBasis basis, float3 v, float roughness, float anisotropy) {
    float3 along = anisotropy >= 0.0 ? basis.b : basis.t;
    float3 across = cross(along, v);
    float3 bent = cross(across, along);
    float stretch = clamp(abs(anisotropy), 0.0, 1.0) * clamp(5.0 * roughness, 0.0, 1.0);
    float3 n = normalize(lerp(basis.n, bent, stretch));
    return rikka_pbr_dominant(n, v, roughness);
}
