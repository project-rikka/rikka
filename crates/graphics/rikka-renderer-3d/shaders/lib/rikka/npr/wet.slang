__exported import rikka.npr.head;
__exported import rikka.wet;

struct RikkaNprWet {
    float3 albedo;
    float film;
    float3 film_normal;
    float film_view;
};

RikkaNprWet rikka_npr_wet(RikkaPixel pixel, float3 albedo, float porosity, float3 normal) {
    RikkaWet w = rikka_wet(pixel.wet, porosity);
    RikkaNprWet r;
    r.albedo = rikka_wet_albedo(albedo, w);
    r.film = w.film;
    r.film_normal = rikka_wet_film_normal(normalize(normal), pixel.geometric_normal, w);
    r.film_view = w.film * rikka_wet_film_sheen(max(dot(r.film_normal, pixel.view), 1e-4));
    return r;
}

float rikka_npr_wet_glint(float3 film_normal, float film, float3 light, float3 view) {
    if (film <= 0.0) {
        return 0.0;
    }
    float lit_side = clamp(dot(film_normal, light) * 4.0, 0.0, 1.0);
    return film * lit_side * rikka_wet_film_glint(dot(film_normal, normalize(light + view)));
}

struct RikkaNprWetFilm {
    float sheen;
    float3 reflection;
};

RikkaNprWetFilm rikka_npr_wet_film(RikkaIndirect indirect, float3 film_normal, float film, float3 view) {
    RikkaNprWetFilm f;
    f.sheen = 0.0;
    f.reflection = float3(0.0);
    if (film > 0.0) {
        f.sheen = film * rikka_wet_sky_sheen(film_normal, view);
        f.reflection = indirect.specular(reflect(-view, film_normal), RIKKA_WET_FILM_ROUGHNESS);
    }
    return f;
}
