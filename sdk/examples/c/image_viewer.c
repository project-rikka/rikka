#include <rikka/rikka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    char *path;
    rk_image *image;
} viewer;

static void show(viewer *state, rk_str path) {
    char *copy = malloc(path.length + 1);
    if (copy == NULL) {
        fputs("image_viewer: out of memory\n", stderr);
        exit(1);
    }
    memcpy(copy, path.data, path.length);
    copy[path.length] = '\0';
    free(state->path);
    rk_image_unref(state->image);
    state->path = copy;
    state->image = rk_image_new_file(path);
}

static void chosen(rk_context *context, rk_status status, const rk_str *paths, size_t count, void *userdata) {
    (void)context;
    if (status == RK_STATUS_OK && count > 0) {
        show(userdata, paths[0]);
    }
}

static void dropped(rk_context *context, const rk_str *paths, size_t count, void *userdata) {
    (void)context;
    if (count > 0) {
        show(userdata, paths[0]);
    }
}

static void choose(rk_context *context, void *userdata) {
    rk_file_chooser *chooser = rk_file_chooser_new(RK_FILE_CHOOSER_PURPOSE_OPEN, RK_STR("Open Picture"));
    rk_file_chooser_add_filter(chooser, RK_STR("Pictures"));
    rk_file_chooser_add_filter_mime_type(chooser, RK_STR("image/*"));
    rk_context_choose_files(context, chooser, chosen, userdata, NULL);
}

static void open_elsewhere(rk_context *context, void *userdata) {
    viewer *state = userdata;
    if (rk_open_file(rk_cstr(state->path)) != RK_STATUS_OK) {
        rk_context_alert(context, RK_STR("Couldn’t Open the Picture"), rk_last_error());
    }
}

static void view(rk_view *view, void *userdata) {
    viewer *state = userdata;
    const char *slash = state->path != NULL ? strrchr(state->path, '/') : NULL;
    const char *name = slash != NULL ? slash + 1 : "";

    rk_column_begin(view);
    rk_fill_width(view);
    rk_fill_height(view);
    rk_on_drop_files(view, dropped, state, NULL);
    {
        rk_row_begin(view);
        rk_gap(view, RK_SPACE_SNUG);
        rk_padding(view, RK_SPACE_NORMAL);
        rk_align_items(view, RK_ALIGN_CENTER);
        {
            rk_button(view, RK_STR("Open Picture…"));
            rk_on_press(view, choose, state, NULL);

            rk_icon_button(view, RK_STR("open_in_new"));
            rk_tooltip(view, RK_STR("Open in Default App"));
            rk_enabled(view, state->path != NULL);
            rk_on_press(view, open_elsewhere, state, NULL);

            rk_text(view, rk_cstr(name));
            rk_text_no_wrap(view);
            rk_grow(view, 1);
        }
        rk_end(view);

        rk_divider(view);

        if (state->image == NULL) {
            rk_status_page_begin(view, RK_STR("image"), RK_STR("Drop a picture here, or open one."));
            rk_grow(view, 1);
            rk_end(view);
        } else {
            rk_image_view(view, state->image);
            rk_image_fit(view, RK_FIT_CONTAIN);
            rk_fill_width(view);
            rk_grow(view, 1);
        }
    }
    rk_end(view);
}

int main(void) {
    viewer state = {0};
    rk_app *app = rk_app_new(RK_STR("org.rikka.examples.image_viewer"));
    rk_window_builder *window = rk_window_builder_new(view, &state, NULL);
    rk_window_builder_set_title(window, RK_STR("Image Viewer"));
    rk_window_builder_set_size(window, 720, 540);
    rk_app_add_window(app, window);

    rk_status status = rk_app_run(app);
    rk_image_unref(state.image);
    free(state.path);
    if (status != RK_STATUS_OK) {
        rk_str error = rk_last_error();
        fprintf(stderr, "image_viewer: %.*s\n", (int)error.length, error.data);
        return 1;
    }
    return 0;
}
