#include <rikka/rikka.h>

#include <stdio.h>

static const char website[] = "https://github.com/project-rikka/rikka";

typedef struct {
    const char *answer;
} dialogs;

static void show_message(rk_context *context, void *userdata) {
    (void)userdata;
    rk_context_alert(context, RK_STR("Hello"), RK_STR("This message was shown with rk_context_alert."));
}

static void answered(rk_context *context, bool confirmed, void *userdata) {
    (void)context;
    dialogs *state = userdata;
    state->answer = confirmed ? "Everything was deleted." : "You kept everything.";
}

static void ask_to_delete(rk_context *context, void *userdata) {
    rk_context_confirm(context, RK_STR("Delete everything?"), RK_STR("This cannot be undone."), RK_STR("Delete"),
                       answered, userdata, NULL);
}

static void visit(rk_context *context, void *userdata) {
    (void)userdata;
    if (rk_open_url(RK_STR(website)) != RK_STATUS_OK) {
        rk_context_alert(context, RK_STR("Couldn’t Open the Website"), rk_last_error());
    }
}

static void view(rk_view *view, void *userdata) {
    dialogs *state = userdata;
    rk_column_begin(view);
    rk_gap(view, RK_SPACE_ROOMY);
    rk_padding(view, RK_SPACE_LOOSE);
    {
        rk_text(view, RK_STR("Dialogs"));
        rk_text_style(view, RK_TEXT_ROLE_TITLE);

        rk_button(view, RK_STR("Show a Message"));
        rk_on_press(view, show_message, state, NULL);

        rk_row_begin(view);
        rk_gap(view, RK_SPACE_NORMAL);
        rk_align_items(view, RK_ALIGN_CENTER);
        {
            rk_button(view, RK_STR("Delete Everything…"));
            rk_button_style(view, RK_BUTTON_VARIANT_DANGER);
            rk_on_press(view, ask_to_delete, state, NULL);

            rk_text(view, rk_cstr(state->answer));
            rk_tone(view, RK_TEXT_TONE_MUTED);
        }
        rk_end(view);

        rk_button(view, RK_STR("Visit the Website"));
        rk_button_style(view, RK_BUTTON_VARIANT_SECONDARY);
        rk_on_press(view, visit, state, NULL);
    }
    rk_end(view);
}

int main(void) {
    dialogs state = {"Nothing was deleted yet."};
    rk_app *app = rk_app_new(RK_STR("org.rikka.examples.dialogs"));
    rk_window_builder *window = rk_window_builder_new(view, &state, NULL);
    rk_window_builder_set_title(window, RK_STR("Dialogs"));
    rk_window_builder_set_size(window, 480, 320);
    rk_app_add_window(app, window);

    rk_status status = rk_app_run(app);
    if (status != RK_STATUS_OK) {
        rk_str error = rk_last_error();
        fprintf(stderr, "dialogs: %.*s\n", (int)error.length, error.data);
        return 1;
    }
    return 0;
}
