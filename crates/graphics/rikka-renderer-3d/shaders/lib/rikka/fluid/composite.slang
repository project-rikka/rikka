__exported import rikka.sky.atmosphere;
__exported import rikka.shading;
__exported import rikka.push;
__exported import rikka.fluid.strand;
__exported import rikka.fluid.style;

static const float REFRACTION_REACH = 0.4;
static const float SCREEN_MARGIN = 0.05;
static const float SHALLOWEST_EXIT = 0.2;
static const float SHADOW_LIFT = 2.0;
static const float FORWARD = 0.6;
static const float BACKWARD = 0.5;
static const int RIM_PARTS = 4;
static const float2 SEEPING = float2(0.04, 0.1);

float backscatter_fraction(float scattering) {
    float reduced = (1.0 - FORWARD) * scattering;
    return (BACKWARD + reduced) / (1.0 + reduced);
}

float3 refracted_direction(float3 eye, float3 normal, float ior) {
    float cos_in = clamp(dot(normal, eye), 0.0, 1.0);
    float3 lateral = normal - eye * cos_in;
    float sin_in = length(lateral);
    if (sin_in < 1e-5) {
        return -eye;
    }
    float deviation = 2.0 * (asin(min(sin_in, 1.0)) - asin(min(sin_in / ior, 1.0)));
    return normalize(-eye * cos(deviation) - lateral / sin_in * sin(deviation));
}

float4 backdrop(RikkaFrameRef frame, float2 uv, int2 pixel, float3 position, float distance,
                float3 eye, float3 normal, float3 out_of, float ior, float path) {
    RikkaViewRef view = frame.view;
    RikkaFluidRef fluid = frame.fluid;
    float4 straight = rikka_fluid_sample(view, rikka_push.slot, uv, 1u);
    float floor_depth = rikka_fetch(rikka_images[fluid.scene_depth], pixel, 0).r;
    bool open = floor_depth <= 0.0;
    float gap = open
        ? 1e30
        : max(rikka_fluid_distance_of(view.projection, floor_depth) - distance, 0.0)
            / max(dot(-eye, view.camera_forward), 0.1);
    float resting = smoothstep(2.5, 1.5, gap / max(path, 1e-5));

    float4 clip;
    if (resting > 0.5) {
        float3 bent = refract(-eye, normal, 1.0 / ior);
        bent = dot(bent, bent) > 0.0 ? bent : -eye;
        float landing = gap * max(dot(eye, normal), 0.0) / max(-dot(bent, normal), 1e-3);
        clip = mul(view.view_projection, float4(position + bent * min(landing, REFRACTION_REACH), 1.0));
    } else if (open) {
        clip = mul(view.view_projection, float4(out_of, 0.0));
    } else {
        float travel = gap / max(dot(out_of, -eye), SHALLOWEST_EXIT);
        clip = mul(view.view_projection, float4(position + out_of * travel, 1.0));
    }

    float4 away = rikka_sky_lit(frame) && open
        ? float4(rikka_sky_specular(frame, out_of, 0.0), straight.a)
        : straight;
    if (clip.w <= 0.0) {
        return away;
    }
    float2 candidate = clip.xy / clip.w * 0.5 + 0.5;
    float2 margins = min(candidate, 1.0 - candidate);
    float inside = smoothstep(0.0, SCREEN_MARGIN, min(margins.x, margins.y));
    candidate = clamp(candidate, float2(0.0), float2(1.0));
    int2 texel = clamp(int2(candidate * view.viewport), int2(0), int2(view.viewport) - 1);
    float depth = rikka_fetch(rikka_images[fluid.scene_depth], texel, 0).r;
    float behind = depth > 0.0 ? rikka_fluid_distance_of(view.projection, depth) : 1e30;
    float unblocked = smoothstep(distance - 2.0 * fluid.radius, distance - fluid.radius, behind);
    float4 seen = lerp(straight, rikka_fluid_sample(view, rikka_push.slot, candidate, 1u), unblocked);
    return lerp(away, seen, inside);
}

float4 rim_reflection(RikkaFrameRef frame, RikkaFluidStrand strand, float3 eye, float f0, float lobe) {
    float4 rim = float4(0.0);
    float covered = 0.0;
    for (int part = 0; part < RIM_PARTS; ++part) {
        float across = strand.across + ((float(part) + 0.5) / float(RIM_PARTS) - 0.5) * strand.pixel;
        if (abs(across) >= 1.0) {
            continue;
        }
        float3 normal = rikka_fluid_strand_normal(strand, eye, across);
        float cosine = max(dot(normal, eye), 1e-3);
        float fresnel = rikka_fresnel(float3(f0), cosine).x;
        rim += float4(rikka_sky_specular(frame, reflect(-eye, normal), lobe) * fresnel, fresnel);
        covered += 1.0;
    }
    return covered > 0.0 ? rim / covered : float4(0.0);
}

float4 rikka_fluid_composite(float4 frag_coord, bool styled) {
    int2 pixel = int2(frag_coord.xy);
    RikkaFrameRef frame = rikka_push.frame;
    RikkaFluidRef fluid = frame.fluid;
    float4 scene = rikka_fetch(rikka_images[rikka_push.slot], pixel, 0);
    float distance = rikka_fetch(rikka_images[fluid.smoothed], pixel, 0).r;
    RikkaViewRef view = frame.view;
    float2 uv = (float2(pixel) + 0.5) * view.inverse_viewport;
    float3 position = float3(0.0);
    float3 eye = float3(0.0, 0.0, 1.0);
    float3 smoothed = eye;
    float3 n = eye;
    float4 scattering = float4(0.0);
    float clouded = 0.0;
    RikkaFluidStrand strand = rikka_fluid_empty_strand();
    if (distance > 0.0) {
        position = rikka_fluid_position(view, uv, distance);
        eye = rikka_fluid_eye(view, position);
        scattering = rikka_fluid_amount(view, fluid, fluid.scattering, uv);
        clouded = 1.0 - exp(-scattering.a);
        smoothed = rikka_fluid_shading_normal(view, fluid, pixel, position, distance, eye, clouded);
        float thin = 1.0 - rikka_fluid_outline(view, fluid, uv, distance).y;
        strand = rikka_fluid_strand(frame, pixel, uv, position, distance, eye, thin);
        n = rikka_fluid_surface_normal(strand, smoothed, eye);
    }
    float3 across = ddx(smoothed);
    float3 down = ddy(smoothed);
    if (distance <= 0.0) {
        return scene;
    }
    float n_dot_v = max(dot(n, eye), 1e-3);

    float4 front = rikka_fluid_front(view, fluid, pixel);
    float roughness = clamp(front.b, 0.02, 1.0);
    float variance = 0.25 * (dot(across, across) + dot(down, down));
    float alpha = roughness * roughness;
    float lobe = sqrt(sqrt(alpha * alpha + min(2.0 * variance, 0.18)));
    float ior = max(front.a, 1.0001);
    float f0 = (ior - 1.0) / (ior + 1.0);
    f0 *= f0;

    float4 thickness = rikka_fluid_amount(view, fluid, fluid.thickness, uv);
    float2 spread = 3.0 * view.inverse_viewport;
    float broad = 0.0;
    [unroll]
    for (int y = -1; y <= 1; ++y) {
        [unroll]
        for (int x = -1; x <= 1; ++x) {
            broad += rikka_fluid_amount(view, fluid, fluid.thickness, uv + float2(float(x), float(y)) * spread).a;
        }
    }
    broad *= 1.0 / 9.0;
    float path = thickness.a;
    float3 optical = thickness.rgb + float3(scattering.a);
    float3 albedo = scattering.a > 1e-5 ? clamp(scattering.rgb / scattering.a, float3(0.0), float3(1.0)) : float3(1.0);
    float coverage = rikka_fluid_coverage(view, fluid, uv, distance, path);

    float3 out_of = refracted_direction(eye, n, ior);
    float4 behind = backdrop(frame, uv, pixel, position, distance, eye, n, out_of, ior, broad);
    float3 kept = exp(-optical);
    float3 transmitted = behind.rgb * kept;

    RikkaShadowPoint s;
    s.world_position = position + n * (SHADOW_LIFT * fluid.radius);
    s.geometric_normal = n;
    s.view_depth = dot(position - view.camera_position, view.camera_forward);
    s.flags = RIKKA_RECEIVES_SHADOW;
    s.rotation = rikka_shadow_rotation(frame, frag_coord.xy);
    s.traced = RIKKA_UNTRACED;

    float wrap = 0.5 * clouded;
    float sheer = exp(-scattering.a);
    float seeping = smoothstep(SEEPING.x, SEEPING.y, clouded * (1.0 - backscatter_fraction(scattering.a)));
    RikkaFluidStyle style = rikka_fluid_style(frame);
    float stylized = styled && clouded > RIKKA_FLUID_CLEAR ? style.stylized : 0.0;
    float3 lighting = float3(0.0);
    float3 backlighting = float3(0.0);
    float3 specular = float3(0.0);
    float3 shining = float3(0.0);
    RikkaLightIter it = rikka_lights_begin(frame, frag_coord.xy, s.view_depth);
    RikkaLight light;
    while (rikka_light_next(frame, it, light)) {
        if (!rikka_light_reaches(it, light)) {
            continue;
        }
        float3 l;
        float attenuation;
        rikka_light_sample(frame, light, position, l, attenuation);
        float n_dot_l = dot(n, l);
        float facing = max(n_dot_l + wrap, 0.0) / (1.0 + wrap) * (1.0 / RIKKA_PI);
        float crossing = seeping
            * lerp(max(-n_dot_l, 0.0) * (1.0 / RIKKA_PI), rikka_henyey_greenstein(FORWARD, dot(-l, eye)), sheer);
        if (attenuation <= 0.0) {
            continue;
        }
        float toward = stylized > 0.0 ? rikka_fluid_toward(style, n_dot_l) : 0.0;
        float shadow = 0.0;
        if (facing + crossing > 0.0 || toward > 0.0) {
            s.world_position += l * (SHADOW_LIFT * fluid.radius);
            shadow = rikka_shadow(frame, light, s);
            s.world_position -= l * (SHADOW_LIFT * fluid.radius);
        }
        float3 radiance = light.color * light.intensity * attenuation;
        float3 lit_front = radiance * (shadow * facing);
        float crisp = shadow;
        if (stylized > 0.0) {
            float shadow_cel = rikka_fluid_cel_shadow(style, shadow);
            float fill = light.kind == RIKKA_LIGHT_DIRECTIONAL ? 1.0 : style.local_fill;
            float3 tone = lerp(style.dark * fill, float3(1.0), toward * shadow_cel);
            lit_front = lerp(lit_front, radiance * tone * (1.0 / RIKKA_PI), stylized);
            crisp = lerp(shadow, shadow_cel, stylized);
        }
        lighting += lit_front;
        if (max(shadow, crisp) <= 0.0) {
            continue;
        }
        float3 incident = radiance * shadow;
        backlighting += incident * crossing;
        shining += incident * (0.25 * rikka_ggx_distribution(max(dot(out_of, l), 0.0), lobe));
        if (n_dot_l > 0.0) {
            float3 h = normalize(l + eye);
            float v_dot_h = max(dot(eye, h), 0.0);
            float fresnel = rikka_fresnel(float3(f0), v_dot_h).x;
            specular += radiance * crisp * fresnel * n_dot_l
                * rikka_ggx_distribution(max(dot(n, h), 0.0), lobe)
                * rikka_smith_visibility(n_dot_v, n_dot_l, lobe);
        }
    }
    float3 sky = rikka_sky_irradiance(frame, n);
    if (stylized > 0.0) {
        sky = lerp(sky, rikka_fluid_anime_sky(frame, style, sky), stylized);
    }
    lighting += sky;

    float2 brdf = rikka_environment_brdf(n_dot_v, lobe);
    float3 reflected = rikka_sky_specular(frame, reflect(-eye, n), lobe) * (f0 * brdf.x + brdf.y);
    float fresnel = f0 + (max(1.0 - lobe, f0) - f0) * pow(1.0 - n_dot_v, 5.0);
    if (strand.thin > 0.0) {
        float4 rim = rim_reflection(frame, strand, eye, f0, sqrt(alpha));
        reflected = lerp(reflected, rim.rgb, strand.thin);
        fresnel = lerp(fresnel, rim.a, strand.thin);
    }
    float3 body = transmitted + shining * kept;
    if (clouded > RIKKA_FLUID_CLEAR) {
        backlighting += seeping * rikka_fluid_sample(view, rikka_push.slot, uv, 1u).rgb;
        float backscattered = backscatter_fraction(scattering.a);
        body += albedo * clouded
            * (backscattered * lighting + (1.0 - backscattered) * exp(-thickness.rgb) * backlighting);
    }
    float3 color = body * (1.0 - fresnel) + reflected + specular;
    float clear = (1.0 - fresnel) * dot(kept, float3(1.0 / 3.0));
    float opacity = 1.0 - (1.0 - behind.a) * clear;
    return lerp(scene, float4(color, opacity), coverage);
}
