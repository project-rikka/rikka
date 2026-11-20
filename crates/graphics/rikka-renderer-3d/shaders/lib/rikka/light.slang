__exported import rikka.common;

static const uint RIKKA_LTC_SIZE = 64u;
static const uint RIKKA_PROFILE_VERTICAL = 128u;
static const uint RIKKA_PROFILE_HORIZONTAL = 64u;

struct RikkaLtc {
    float3x3 inverse;
    float magnitude;
    float fresnel;
};

static const uint RIKKA_LTC_LINEAR_CLAMP = 1u;

RikkaLtcEntry rikka_ltc_mix(RikkaFrameRef frame, float2 coordinate) {
    float2 uv = (clamp(coordinate, float2(0.0), float2(1.0)) * float(RIKKA_LTC_SIZE - 1u) + 0.5)
        * (1.0 / float(RIKKA_LTC_SIZE));
    SamplerState linear = rikka_samplers[RIKKA_LTC_LINEAR_CLAMP];
    RikkaLtcEntry mixed;
    mixed.inverse = rikka_images[frame.look.scene.ltc_inverse].SampleLevel(linear, uv, 0.0);
    mixed.amplitude = rikka_images[frame.look.scene.ltc_amplitude].SampleLevel(linear, uv, 0.0);
    return mixed;
}

RikkaLtc rikka_ltc(RikkaFrameRef frame, float roughness, float n_dot_v) {
    RikkaLtcEntry entry = rikka_ltc_mix(frame, float2(roughness, sqrt(max(1.0 - n_dot_v, 0.0))));
    float4 m = entry.inverse;
    RikkaLtc ltc;
    ltc.inverse = rikka_columns(float3(m.x, 0.0, m.y), float3(0.0, 1.0, 0.0), float3(m.z, 0.0, m.w));
    ltc.magnitude = entry.amplitude.x;
    ltc.fresnel = entry.amplitude.y;
    return ltc;
}

float rikka_ltc_clipped(RikkaFrameRef frame, float cosine, float form_factor) {
    if (form_factor <= 0.0) {
        return 0.0;
    }
    float scale = rikka_ltc_mix(frame, float2(cosine * 0.5 + 0.5, form_factor)).amplitude.z;
    return form_factor * scale;
}

float3 rikka_ltc_edge(float3 a, float3 b) {
    float x = dot(a, b);
    float y = abs(x);
    float numerator = 0.8543985 + (0.4965155 + 0.0145206 * y) * y;
    float denominator = 3.4175940 + (4.1616724 + y) * y;
    float v = numerator / denominator;
    float theta_over_sine = x > 0.0 ? v : 0.5 * rsqrt(max(1.0 - x * x, 1e-7)) - v;
    return cross(a, b) * theta_over_sine;
}

struct RikkaQuad {
    float3 center;
    float3 across;
    float3 up;
};

float3 rikka_quad_vector(RikkaQuad quad, float3x3 to_lobe) {
    float3 l0 = normalize(mul(to_lobe, quad.center - quad.across - quad.up));
    float3 l1 = normalize(mul(to_lobe, quad.center + quad.across - quad.up));
    float3 l2 = normalize(mul(to_lobe, quad.center + quad.across + quad.up));
    float3 l3 = normalize(mul(to_lobe, quad.center - quad.across + quad.up));
    return -(rikka_ltc_edge(l0, l1) + rikka_ltc_edge(l1, l2) + rikka_ltc_edge(l2, l3) + rikka_ltc_edge(l3, l0));
}

float3 rikka_cubic_roots(float4 coefficients) {
    coefficients.xyz /= coefficients.w;
    coefficients.yz /= 3.0;
    float a = coefficients.w;
    float b = coefficients.z;
    float c = coefficients.y;
    float d = coefficients.x;
    float3 delta = float3(
        -coefficients.z * coefficients.z + coefficients.y,
        -coefficients.y * coefficients.z + coefficients.x,
        dot(float2(coefficients.z, -coefficients.y), coefficients.xy));
    float discriminant = dot(float2(4.0 * delta.x, -delta.y), delta.zy);
    float2 large;
    {
        float c_a = delta.x;
        float d_a = -2.0 * b * delta.x + delta.y;
        float theta = atan2(sqrt(max(discriminant, 0.0)), -d_a) / 3.0;
        float x1 = 2.0 * sqrt(max(-c_a, 0.0)) * cos(theta);
        float x3 = 2.0 * sqrt(max(-c_a, 0.0)) * cos(theta + (2.0 / 3.0) * RIKKA_PI);
        float xl = (x1 + x3) > 2.0 * b ? x1 : x3;
        large = float2(xl - b, a);
    }
    float2 small;
    {
        float c_d = delta.z;
        float d_d = -d * delta.y + 2.0 * c * delta.z;
        float theta = atan2(d * sqrt(max(discriminant, 0.0)), -d_d) / 3.0;
        float x1 = 2.0 * sqrt(max(-c_d, 0.0)) * cos(theta);
        float x3 = 2.0 * sqrt(max(-c_d, 0.0)) * cos(theta + (2.0 / 3.0) * RIKKA_PI);
        float xs = x1 + x3 < 2.0 * c ? x1 : x3;
        small = float2(-d, xs + c);
    }
    float e = large.y * small.y;
    float f = -large.x * small.y - large.y * small.x;
    float g = large.x * small.x;
    float2 middle = float2(c * f - b * g, -b * f + c * e);
    float3 roots = float3(small.x / small.y, middle.x / middle.y, large.x / large.y);
    if (roots.x < roots.y && roots.x < roots.z) {
        roots = roots.yxz;
    } else if (roots.z < roots.x && roots.z < roots.y) {
        roots = roots.xzy;
    }
    return roots;
}

float4 rikka_ellipse_vector(float3 center, float3 axis_a, float3 axis_b) {
    float d11 = dot(axis_a, axis_a);
    float d22 = dot(axis_b, axis_b);
    float d12 = dot(axis_a, axis_b);
    float a;
    float b;
    float3 v1;
    float3 v2;
    if (abs(d12) / sqrt(d11 * d22) > 1e-4) {
        float trace = d11 + d22;
        float determinant = sqrt(max(-d12 * d12 + d11 * d22, 0.0));
        float u = 0.5 * sqrt(max(trace - 2.0 * determinant, 0.0));
        float v = 0.5 * sqrt(trace + 2.0 * determinant);
        float largest = (u + v) * (u + v);
        float smallest = (u - v) * (u - v);
        float3 major;
        float3 minor;
        if (d11 > d22) {
            major = d12 * axis_a + (largest - d11) * axis_b;
            minor = d12 * axis_a + (smallest - d11) * axis_b;
        } else {
            major = d12 * axis_b + (largest - d22) * axis_a;
            minor = d12 * axis_b + (smallest - d22) * axis_a;
        }
        a = 1.0 / largest;
        b = 1.0 / max(smallest, 1e-12);
        v1 = normalize(major);
        v2 = normalize(minor);
    } else {
        a = 1.0 / d11;
        b = 1.0 / d22;
        v1 = axis_a * sqrt(a);
        v2 = axis_b * sqrt(b);
    }
    float3 v3 = cross(v1, v2);
    if (dot(center, v3) < 0.0) {
        v3 = -v3;
    }
    float l = dot(v3, center);
    float x0 = dot(v1, center) / l;
    float y0 = dot(v2, center) / l;
    a *= l * l;
    b *= l * l;
    float c0 = a * b;
    float c1 = a * b * (1.0 + x0 * x0 + y0 * y0) - a - b;
    float c2 = 1.0 - a * (1.0 + x0 * x0) - b * (1.0 + y0 * y0);
    float3 roots = rikka_cubic_roots(float4(c0, c1, c2, 1.0));
    float e1 = roots.x;
    float e2 = roots.y;
    float e3 = roots.z;
    float3 average = float3(a * x0 / (a - e2), b * y0 / (b - e2), 1.0);
    average = normalize(v1 * average.x + v2 * average.y + v3 * average.z);
    float l1 = sqrt(max(-e2 / e3, 0.0));
    float l2 = sqrt(max(-e2 / e1, 0.0));
    float form_factor = l1 * l2 * rsqrt((1.0 + l1 * l1) * (1.0 + l2 * l2));
    return float4(average, form_factor);
}

struct RikkaAreaShape {
    float3 center;
    float3 across;
    float3 up;
    bool ellipse;
    bool lit;
};

RikkaAreaShape rikka_area_shape(RikkaLight light, float3 position) {
    RikkaAreaShape shape;
    float3 offset = light.position - position;
    shape.center = offset;
    shape.ellipse = light.kind == RIKKA_LIGHT_DISC || light.kind == RIKKA_LIGHT_SPHERE;
    shape.lit = true;
    if (light.kind == RIKKA_LIGHT_SPHERE) {
        float distance = max(length(offset), light.extent.x * 1.001);
        float3 facing = -offset / max(length(offset), 1e-6);
        float3 across = normalize(cross(facing, abs(facing.y) < 0.99 ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0)));
        float radius = light.extent.x * distance * rsqrt(max(distance * distance - light.extent.x * light.extent.x, 1e-8));
        shape.across = across * radius;
        shape.up = cross(facing, across) * radius;
        return shape;
    }
    if (light.kind == RIKKA_LIGHT_TUBE) {
        float3 axis = light.direction;
        float3 away = -offset - axis * dot(-offset, axis);
        float3 facing = dot(away, away) > 1e-10 ? normalize(away) : axis.yzx;
        shape.across = axis * (light.extent.x + 0.25 * RIKKA_PI * light.extent.y);
        shape.up = cross(facing, axis) * light.extent.y;
        return shape;
    }
    float3 normal = light.direction;
    float3 tangent = light.tangent;
    float3 bitangent = cross(normal, tangent);
    shape.across = tangent * light.extent.x;
    shape.up = bitangent * light.extent.y;
    bool front = dot(-offset, normal) > 0.0;
    shape.lit = front || (light.flags & RIKKA_LIGHT_TWO_SIDED) != 0u;
    if (!front) {
        shape.up = -shape.up;
    }
    return shape;
}

float4 rikka_area_vector(RikkaAreaShape shape, float3x3 to_lobe) {
    if (!shape.lit) {
        return float4(0.0, 0.0, 1.0, 0.0);
    }
    if (shape.ellipse) {
        return rikka_ellipse_vector(mul(to_lobe, shape.center), mul(to_lobe, shape.across), mul(to_lobe, shape.up));
    }
    RikkaQuad quad;
    quad.center = shape.center;
    quad.across = shape.across;
    quad.up = shape.up;
    float3 summed = rikka_quad_vector(quad, to_lobe);
    float form_factor = length(summed);
    return float4(form_factor > 0.0 ? summed / form_factor : float3(0.0, 0.0, 1.0), form_factor);
}

float rikka_light_window(RikkaLight light, float distance) {
    float ratio = distance / max(light.range, 1e-4);
    float window = clamp(1.0 - ratio * ratio * ratio * ratio, 0.0, 1.0);
    return window * window;
}

float3x3 rikka_shading_frame(float3 normal, float3 view) {
    float3 tangent = view - normal * dot(view, normal);
    float length_squared = dot(tangent, tangent);
    tangent = length_squared > 1e-8 ? tangent * rsqrt(length_squared) : normal.yzx;
    tangent = normalize(tangent - normal * dot(tangent, normal));
    float3 bitangent = cross(normal, tangent);
    return float3x3(tangent, bitangent, normal);
}

struct RikkaLightSample {
    RikkaLight light;
    float3 direction;
    float attenuation;
    bool leading;
    float3 position;
    float4 vector_form_factor;
    float window;

    bool area() {
        return light.kind >= RIKKA_LIGHT_RECT;
    }

    float3 radiance() {
        return light.color * light.intensity;
    }

    float irradiance(RikkaFrameRef frame, float3 normal) {
        if (!area()) {
            return attenuation * max(dot(normal, direction), 0.0);
        }
        float cosine = dot(normal, vector_form_factor.xyz);
        return RIKKA_PI * window * rikka_ltc_clipped(frame, cosine, vector_form_factor.w);
    }

    float specular(RikkaFrameRef frame, RikkaLtc ltc, float3 normal, float3 view) {
        if (!area() || window <= 0.0) {
            return 0.0;
        }
        float3x3 to_lobe = mul(ltc.inverse, rikka_shading_frame(normal, view));
        float4 lobe = rikka_area_vector(rikka_area_shape(light, position), to_lobe);
        return window * rikka_ltc_clipped(frame, lobe.z, lobe.w);
    }
};

float rikka_light_profile_texel(RikkaProfileRef table, uint row, uint column) {
    return table[row * RIKKA_PROFILE_HORIZONTAL + column];
}

float rikka_light_profile(RikkaFrameRef frame, RikkaLight light, float3 from_light) {
    if (light.profile == RIKKA_NONE) {
        return 1.0;
    }
    float3 nadir = light.direction;
    float3 zero = light.tangent;
    float3 quarter = cross(zero, nadir);
    float vertical = acos(clamp(dot(from_light, nadir), -1.0, 1.0));
    float horizontal = atan2(dot(from_light, quarter), dot(from_light, zero));
    float v = vertical * (1.0 / RIKKA_PI) * float(RIKKA_PROFILE_VERTICAL - 1u);
    uint row = min(uint(v), RIKKA_PROFILE_VERTICAL - 2u);
    float down = v - float(row);
    float h = fract(horizontal * (0.5 / RIKKA_PI) + 1.0) * float(RIKKA_PROFILE_HORIZONTAL);
    uint column = min(uint(h), RIKKA_PROFILE_HORIZONTAL - 1u);
    uint next = (column + 1u) % RIKKA_PROFILE_HORIZONTAL;
    float across = h - float(column);
    RikkaProfileRef table = frame.profiles + light.profile * RIKKA_PROFILE_VERTICAL * RIKKA_PROFILE_HORIZONTAL;
    float upper = lerp(rikka_light_profile_texel(table, row, column), rikka_light_profile_texel(table, row, next), across);
    float lower = lerp(rikka_light_profile_texel(table, row + 1u, column), rikka_light_profile_texel(table, row + 1u, next), across);
    return lerp(upper, lower, down);
}

RikkaLightSample rikka_sample_light(RikkaFrameRef frame, RikkaLight light, float3 position) {
    RikkaLightSample s;
    s.light = light;
    s.leading = (light.flags & RIKKA_LIGHT_LEADING) != 0u;
    s.position = position;
    s.vector_form_factor = float4(0.0, 0.0, 1.0, 0.0);
    s.window = 1.0;
    if (light.kind == RIKKA_LIGHT_DIRECTIONAL) {
        s.direction = -light.direction;
        s.attenuation = 1.0;
        return s;
    }
    float3 offset = light.position - position;
    float distance_squared = dot(offset, offset);
    float distance = sqrt(max(distance_squared, 1e-8));
    s.window = rikka_light_window(light, distance);
    if (light.kind >= RIKKA_LIGHT_RECT) {
        float4 world;
        if (light.kind == RIKKA_LIGHT_SPHERE) {
            float radius = light.extent.x;
            world = float4(offset / distance, min(radius * radius / max(distance_squared, 1e-8), 1.0));
        } else {
            world = rikka_area_vector(rikka_area_shape(light, position), float3x3(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0));
        }
        s.vector_form_factor = world;
        s.direction = world.xyz;
        s.attenuation = RIKKA_PI * world.w * s.window;
        return s;
    }
    s.direction = offset / distance;
    float falloff = 1.0 / max(distance_squared, 1e-4);
    s.attenuation = falloff * s.window * rikka_light_profile(frame, light, -s.direction);
    if (light.kind == RIKKA_LIGHT_SPOT) {
        float cosine = dot(-s.direction, light.direction);
        float cone = clamp(cosine * light.cone_scale + light.cone_offset, 0.0, 1.0);
        s.attenuation *= cone * cone;
    }
    return s;
}

float rikka_light_power(RikkaLight light) {
    switch (light.kind) {
    case RIKKA_LIGHT_RECT:
        return light.intensity * 4.0 * light.extent.x * light.extent.y;
    case RIKKA_LIGHT_DISC:
    case RIKKA_LIGHT_SPHERE:
        return light.intensity * RIKKA_PI * light.extent.x * light.extent.x;
    case RIKKA_LIGHT_TUBE:
        return light.intensity * (4.0 * light.extent.x * light.extent.y + RIKKA_PI * light.extent.y * light.extent.y);
    default:
        return light.intensity;
    }
}
