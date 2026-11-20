#include <rikka/rikka.h>

#include <stdio.h>

static const rk_color black = {0, 0, 0, 1};

typedef struct {
    rk_video *video;
    bool scrubbing;
    double scrub;
} player;

static void clock_text(double seconds, char *shown, size_t size) {
    int whole = (int)seconds;
    snprintf(shown, size, "%d:%02d", whole / 60, whole % 60);
}

static void chosen(rk_context *context, rk_status status, const rk_str *paths, size_t count, void *userdata) {
    (void)context;
    player *state = userdata;
    if (status != RK_STATUS_OK || count == 0) {
        return;
    }
    if (state->video != NULL) {
        rk_video_stop(state->video);
        rk_video_unref(state->video);
    }
    state->scrubbing = false;
    state->video = rk_video_new(paths[0]);
    rk_video_play(state->video);
}

static void choose(rk_context *context, void *userdata) {
    rk_file_chooser *chooser = rk_file_chooser_new(RK_FILE_CHOOSER_PURPOSE_OPEN, RK_STR("Open Video"));
    rk_file_chooser_add_filter(chooser, RK_STR("Videos"));
    rk_file_chooser_add_filter_mime_type(chooser, RK_STR("video/*"));
    rk_context_choose_files(context, chooser, chosen, userdata, NULL);
}

static void play_pause(rk_context *context, void *userdata) {
    (void)context;
    player *state = userdata;
    if (rk_video_is_playing(state->video)) {
        rk_video_pause(state->video);
    } else {
        rk_video_play(state->video);
    }
}

static void stop(rk_context *context, void *userdata) {
    (void)context;
    rk_video_stop(((player *)userdata)->video);
}

static void scrubbing(rk_context *context, double value, void *userdata) {
    (void)context;
    player *state = userdata;
    state->scrubbing = true;
    state->scrub = value;
}

static void seek(rk_context *context, double value, void *userdata) {
    (void)context;
    player *state = userdata;
    state->scrubbing = false;
    rk_video_seek(state->video, value);
}

static void set_volume(rk_context *context, double value, void *userdata) {
    (void)context;
    rk_video_set_volume(((player *)userdata)->video, value);
}

static void controls(rk_view *view, player *state) {
    rk_video *video = state->video;
    double duration = rk_video_duration(video);
    double position = state->scrubbing ? state->scrub : rk_video_position(video);
    rk_slider(view, position);
    rk_range(view, 0, duration > 1 ? duration : 1);
    rk_enabled(view, duration > 0);
    rk_on_value(view, scrubbing, state, NULL);
    rk_on_value_commit(view, seek, state, NULL);

    char elapsed[32];
    char total[32];
    char times[80];
    clock_text(position, elapsed, sizeof elapsed);
    clock_text(duration, total, sizeof total);
    snprintf(times, sizeof times, "%s / %s", elapsed, total);
    bool playing = rk_video_is_playing(video);

    rk_row_begin(view);
    rk_gap(view, RK_SPACE_SNUG);
    rk_align_items(view, RK_ALIGN_CENTER);
    {
        rk_icon_button(view, playing ? RK_STR("pause") : RK_STR("play_arrow"));
        rk_tooltip(view, playing ? RK_STR("Pause") : RK_STR("Play"));
        rk_enabled(view, rk_video_state(video) != RK_PLAYBACK_STATE_FAILED);
        rk_on_press(view, play_pause, state, NULL);

        rk_icon_button(view, RK_STR("stop"));
        rk_tooltip(view, RK_STR("Stop"));
        rk_on_press(view, stop, state, NULL);

        rk_text(view, rk_cstr(times));
        rk_text_monospace(view);

        rk_spacer(view);

        rk_icon(view, RK_STR("volume_up"));
        rk_slider(view, rk_video_volume(video));
        rk_width(view, 120);
        rk_on_value(view, set_volume, state, NULL);

        rk_button(view, RK_STR("Open Video…"));
        rk_button_style(view, RK_BUTTON_VARIANT_SECONDARY);
        rk_on_press(view, choose, state, NULL);
    }
    rk_end(view);
}

static void view(rk_view *view, void *userdata) {
    player *state = userdata;
    if (state->video == NULL) {
        rk_status_page_begin(view, RK_STR("movie"), RK_STR("Open a video to play it."));
        rk_fill_width(view);
        rk_fill_height(view);
        {
            rk_button(view, RK_STR("Open Video…"));
            rk_on_press(view, choose, state, NULL);
        }
        rk_end(view);
        return;
    }

    rk_column_begin(view);
    rk_fill_width(view);
    rk_fill_height(view);
    {
        rk_video_view(view, state->video);
        rk_image_fit(view, RK_FIT_CONTAIN);
        rk_background(view, black);
        rk_fill_width(view);
        rk_grow(view, 1);

        rk_column_begin(view);
        rk_gap(view, RK_SPACE_SNUG);
        rk_padding(view, RK_SPACE_NORMAL);
        {
            rk_string message = rk_video_error_message(state->video);
            if (message.length > 0) {
                rk_str shown = {message.data, message.length};
                rk_banner(view, shown);
            }
            rk_string_free(message);
            controls(view, state);
        }
        rk_end(view);
    }
    rk_end(view);
}

int main(void) {
    player state = {0};
    rk_app *app = rk_app_new(RK_STR("org.rikka.examples.video_player"));
    rk_window_builder *window = rk_window_builder_new(view, &state, NULL);
    rk_window_builder_set_title(window, RK_STR("Video Player"));
    rk_window_builder_set_size(window, 800, 560);
    rk_app_add_window(app, window);

    rk_status status = rk_app_run(app);
    rk_video_unref(state.video);
    if (status != RK_STATUS_OK) {
        rk_str error = rk_last_error();
        fprintf(stderr, "video_player: %.*s\n", (int)error.length, error.data);
        return 1;
    }
    return 0;
}
