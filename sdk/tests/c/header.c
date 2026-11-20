#include <rikka/rikka.h>

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(rk_str) == 2 * sizeof(void *), "rk_str is a pointer and a length");
_Static_assert(sizeof(rk_string) == 2 * sizeof(void *), "rk_string is a pointer and a length");
_Static_assert(sizeof(rk_color) == 4 * sizeof(float), "rk_color is four floats");
_Static_assert(sizeof(rk_status) == 4, "enums are 32 bits wide");
_Static_assert(sizeof(rk_key_code) == 4, "enums are 32 bits wide");
_Static_assert(sizeof(rk_modifiers) == 4, "flags are 32 bits wide");
#endif

static void declare(rk_view *view, void *userdata) {
    (void)userdata;
    rk_column_begin(view);
    rk_gap(view, RK_SPACE_SNUG);
    rk_text(view, RK_STR("literal"));
    rk_text(view, rk_cstr("terminated"));
    rk_end(view);
}

int main(void) {
    rk_window_builder *builder = rk_window_builder_new(declare, NULL, NULL);
    rk_window_builder_free(builder);
    return rk_version() == RK_HEADER_VERSION ? 0 : 1;
}
