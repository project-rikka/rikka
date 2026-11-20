#include <rikka/rikka.h>

#include <type_traits>

static_assert(std::is_standard_layout<rk_str>::value, "rk_str is a C struct");
static_assert(std::is_trivially_copyable<rk_color>::value, "rk_color is a C struct");
static_assert(sizeof(rk_status) == 4, "enums are 32 bits wide");

static void declare(rk_view *view, void *) {
    rk_row_begin(view);
    rk_button(view, RK_STR("literal"));
    rk_tooltip(view, rk_cstr("terminated"));
    rk_end(view);
}

int main() {
    rk_window_builder *builder = rk_window_builder_new(declare, nullptr, nullptr);
    rk_window_builder_free(builder);
    return rk_version() == RK_HEADER_VERSION ? 0 : 1;
}
