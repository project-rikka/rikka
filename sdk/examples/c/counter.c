#include <rikka/rikka.h>

#include <stdio.h>

typedef struct {
    long long count;
} counter;

static void add(rk_context *context, void *userdata) {
    (void)context;
    ((counter *)userdata)->count++;
}

static void remove_one(rk_context *context, void *userdata) {
    (void)context;
    ((counter *)userdata)->count--;
}

static void view(rk_view *view, void *userdata) {
    counter *state = userdata;
    char shown[32];
    snprintf(shown, sizeof shown, "%lld", state->count);

    rk_column_begin(view);
    rk_gap(view, RK_SPACE_ROOMY);
    rk_padding(view, RK_SPACE_LOOSE);
    {
        rk_text(view, rk_cstr(shown));
        rk_text_style(view, RK_TEXT_ROLE_DISPLAY);

        rk_row_begin(view);
        rk_gap(view, RK_SPACE_SNUG);
        {
            rk_button(view, RK_STR("Add"));
            rk_on_press(view, add, state, NULL);

            rk_button(view, RK_STR("Remove"));
            rk_button_style(view, RK_BUTTON_VARIANT_SECONDARY);
            rk_enabled(view, state->count > 0);
            rk_on_press(view, remove_one, state, NULL);
        }
        rk_end(view);
    }
    rk_end(view);
}

int main(void) {
    counter state = {0};
    rk_app *app = rk_app_new(RK_STR("org.rikka.examples.counter"));
    rk_window_builder *window = rk_window_builder_new(view, &state, NULL);
    rk_window_builder_set_title(window, RK_STR("Counter"));
    rk_window_builder_set_size(window, 360, 240);
    rk_app_add_window(app, window);

    rk_status status = rk_app_run(app);
    if (status != RK_STATUS_OK) {
        rk_str error = rk_last_error();
        fprintf(stderr, "counter: %.*s\n", (int)error.length, error.data);
        return 1;
    }
    return 0;
}
