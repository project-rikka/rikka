__exported import rikka.effect;

struct Bands {
    [Range(-1.0, 1.0)] float red;
    [Range(-1.0, 1.0)] float orange;
    [Range(-1.0, 1.0)] float yellow;
    [Range(-1.0, 1.0)] float green;
    [Range(-1.0, 1.0)] float aqua;
    [Range(-1.0, 1.0)] float blue;
    [Range(-1.0, 1.0)] float purple;
    [Range(-1.0, 1.0)] float magenta;
};

struct Parameters {
    [Group("hue")] Bands hue;
    [Group("saturation")] Bands saturation;
    [Group("luminance")] Bands luminance;
};

static const int RIKKA_HSL_BANDS = 8;
static const float RIKKA_HSL_CENTRES[8] = { 29.0, 58.0, 105.0, 142.0, 195.0, 255.0, 295.0, 330.0 };
static const float RIKKA_HSL_HUE_SHIFT = 30.0;
static const float RIKKA_HSL_LIGHTNESS = 0.25;
static const float RIKKA_HSL_CHROMA_KNEE = 0.06;

float rikka_hsl_band(Bands bands, int index) {
    switch (index) {
    case 0: return bands.red;
    case 1: return bands.orange;
    case 2: return bands.yellow;
    case 3: return bands.green;
    case 4: return bands.aqua;
    case 5: return bands.blue;
    case 6: return bands.purple;
    default: return bands.magenta;
    }
}

float rikka_hsl_mix(Bands bands, float hue) {
    int upper = 0;
    for (int index = 0; index < RIKKA_HSL_BANDS; ++index) {
        if (hue >= RIKKA_HSL_CENTRES[index]) {
            upper = index + 1;
        }
    }
    int below = (upper + RIKKA_HSL_BANDS - 1) % RIKKA_HSL_BANDS;
    int above = upper % RIKKA_HSL_BANDS;
    float start = RIKKA_HSL_CENTRES[below];
    float span = rikka_mod(RIKKA_HSL_CENTRES[above] - start + 360.0, 360.0);
    float t = smoothstep(0.0, 1.0, rikka_mod(hue - start + 360.0, 360.0) / max(span, 1e-3));
    return lerp(rikka_hsl_band(bands, below), rikka_hsl_band(bands, above), t);
}

float3 rikka_hsl_adjust(Parameters parameters, float3 linear) {
    float3 lab = rikka_effect_to_oklab(max(linear, float3(0.0)));
    float chroma = length(lab.yz);
    if (chroma < 1e-5) {
        return linear;
    }
    float hue = rikka_mod(degrees(atan2(lab.z, lab.y)) + 360.0, 360.0);
    float weight = smoothstep(0.0, RIKKA_HSL_CHROMA_KNEE, chroma);
    float shift = rikka_hsl_mix(parameters.hue, hue) * RIKKA_HSL_HUE_SHIFT * weight;
    float saturation = 1.0 + rikka_hsl_mix(parameters.saturation, hue) * weight;
    float lightness = rikka_hsl_mix(parameters.luminance, hue) * weight;
    float angle = radians(hue + shift);
    float adjusted = chroma * max(saturation, 0.0);
    lab.x = lab.x * (1.0 + lightness * RIKKA_HSL_LIGHTNESS * smoothstep(0.0, 0.2, chroma));
    lab.yz = float2(cos(angle), sin(angle)) * adjusted;
    return max(rikka_effect_from_oklab(lab), float3(0.0));
}
