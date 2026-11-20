#include <rikka/rikka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    char **tracks;
    size_t count;
    size_t current;
    rk_sound *sound;
    bool scrubbing;
    double scrub;
} player;

static void clock_text(double seconds, char *shown, size_t size) {
    int whole = (int)seconds;
    snprintf(shown, size, "%d:%02d", whole / 60, whole % 60);
}

static rk_str stem(const char *path) {
    const char *slash = strrchr(path, '/');
    const char *name = slash != NULL ? slash + 1 : path;
    const char *dot = strrchr(name, '.');
    size_t length = dot != NULL && dot != name ? (size_t)(dot - name) : strlen(name);
    rk_str shown = {name, length};
    return shown;
}

static void forget_tracks(player *state) {
    for (size_t index = 0; index < state->count; index++) {
        free(state->tracks[index]);
    }
    free(state->tracks);
    state->tracks = NULL;
    state->count = 0;
}

static void next(rk_context *context, void *userdata);

static void start(player *state, rk_context *context, size_t index) {
    if (state->sound != NULL) {
        rk_sound_stop(state->sound);
        rk_sound_unref(state->sound);
    }
    state->current = index;
    state->scrubbing = false;
    state->sound = rk_sound_new(rk_cstr(state->tracks[index]));
    rk_sound_on_end(state->sound, rk_context_window(context), next, state, NULL);
    rk_sound_play(state->sound);
}

static void next(rk_context *context, void *userdata) {
    player *state = userdata;
    if (state->current + 1 < state->count) {
        start(state, context, state->current + 1);
    }
}

static void previous(rk_context *context, void *userdata) {
    player *state = userdata;
    start(state, context, state->current - 1);
}

static void pick(rk_context *context, size_t index, void *userdata) {
    start(userdata, context, index);
}

static void *allocated(void *memory) {
    if (memory == NULL) {
        fputs("music_player: out of memory\n", stderr);
        exit(1);
    }
    return memory;
}

static void chosen(rk_context *context, rk_status status, const rk_str *paths, size_t count, void *userdata) {
    player *state = userdata;
    if (status != RK_STATUS_OK) {
        return;
    }
    forget_tracks(state);
    state->tracks = allocated(calloc(count, sizeof *state->tracks));
    for (size_t index = 0; index < count; index++) {
        state->tracks[index] = allocated(calloc(paths[index].length + 1, 1));
        memcpy(state->tracks[index], paths[index].data, paths[index].length);
    }
    state->count = count;
    start(state, context, 0);
}

static void choose(rk_context *context, void *userdata) {
    rk_file_chooser *chooser = rk_file_chooser_new(RK_FILE_CHOOSER_PURPOSE_OPEN, RK_STR("Choose Music"));
    rk_file_chooser_set_multiple(chooser, true);
    rk_file_chooser_add_filter(chooser, RK_STR("Music"));
    rk_file_chooser_add_filter_mime_type(chooser, RK_STR("audio/*"));
    rk_context_choose_files(context, chooser, chosen, userdata, NULL);
}

static void play_pause(rk_context *context, void *userdata) {
    (void)context;
    player *state = userdata;
    if (rk_sound_is_playing(state->sound)) {
        rk_sound_pause(state->sound);
    } else {
        rk_sound_play(state->sound);
    }
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
    rk_sound_seek(state->sound, value);
}

static void repeat(rk_context *context, bool on, void *userdata) {
    (void)context;
    rk_sound_set_looping(((player *)userdata)->sound, on);
}

static void set_volume(rk_context *context, double value, void *userdata) {
    (void)context;
    rk_sound_set_volume(((player *)userdata)->sound, value);
}

static void controls(rk_view *view, player *state) {
    rk_sound *sound = state->sound;
    if (rk_sound_state(sound) == RK_PLAYBACK_STATE_FAILED) {
        rk_string message = rk_sound_error_message(sound);
        rk_str shown = {message.data, message.length};
        rk_banner(view, shown);
        rk_string_free(message);
        return;
    }
    double duration = rk_sound_duration(sound);
    double position = state->scrubbing ? state->scrub : rk_sound_position(sound);
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
    bool playing = rk_sound_is_playing(sound);

    rk_row_begin(view);
    rk_gap(view, RK_SPACE_SNUG);
    rk_align_items(view, RK_ALIGN_CENTER);
    {
        rk_icon_button(view, RK_STR("skip_previous"));
        rk_tooltip(view, RK_STR("Previous"));
        rk_enabled(view, state->current > 0);
        rk_on_press(view, previous, state, NULL);

        rk_icon_button(view, playing ? RK_STR("pause") : RK_STR("play_arrow"));
        rk_tooltip(view, playing ? RK_STR("Pause") : RK_STR("Play"));
        rk_on_press(view, play_pause, state, NULL);

        rk_icon_button(view, RK_STR("skip_next"));
        rk_tooltip(view, RK_STR("Next"));
        rk_enabled(view, state->current + 1 < state->count);
        rk_on_press(view, next, state, NULL);

        rk_text(view, rk_cstr(times));
        rk_text_monospace(view);

        rk_spacer(view);

        rk_checkbox(view, rk_sound_looping(sound) ? RK_CHECK_STATE_CHECKED : RK_CHECK_STATE_UNCHECKED, RK_STR("Repeat"));
        rk_on_toggle(view, repeat, state, NULL);

        rk_icon(view, RK_STR("volume_up"));
        rk_slider(view, rk_sound_volume(sound));
        rk_width(view, 120);
        rk_on_value(view, set_volume, state, NULL);
    }
    rk_end(view);
}

static void view(rk_view *view, void *userdata) {
    player *state = userdata;
    rk_column_begin(view);
    rk_gap(view, RK_SPACE_ROOMY);
    rk_padding(view, RK_SPACE_LOOSE);
    rk_fill_width(view);
    rk_fill_height(view);
    {
        rk_row_begin(view);
        rk_gap(view, RK_SPACE_SNUG);
        rk_align_items(view, RK_ALIGN_CENTER);
        {
            rk_text(view, RK_STR("Music"));
            rk_text_style(view, RK_TEXT_ROLE_TITLE);
            rk_grow(view, 1);

            rk_button(view, RK_STR("Choose Music…"));
            rk_on_press(view, choose, state, NULL);
        }
        rk_end(view);

        if (state->count == 0) {
            rk_text(view, RK_STR("Choose one or more music files to play them in order."));
            rk_tone(view, RK_TEXT_TONE_MUTED);
        } else {
            rk_list_begin(view);
            rk_selection(view, state->current);
            rk_on_select(view, pick, state, NULL);
            rk_grow(view, 1);
            for (size_t index = 0; index < state->count; index++) {
                rk_list_item(view, stem(state->tracks[index]));
                rk_leading_icon(view, RK_STR("music_note"));
            }
            rk_end(view);

            if (state->sound != NULL) {
                controls(view, state);
            }
        }
    }
    rk_end(view);
}

int main(void) {
    player state = {0};
    rk_app *app = rk_app_new(RK_STR("org.rikka.examples.music_player"));
    rk_window_builder *window = rk_window_builder_new(view, &state, NULL);
    rk_window_builder_set_title(window, RK_STR("Music Player"));
    rk_window_builder_set_size(window, 560, 520);
    rk_app_add_window(app, window);

    rk_status status = rk_app_run(app);
    rk_sound_unref(state.sound);
    forget_tracks(&state);
    if (status != RK_STATUS_OK) {
        rk_str error = rk_last_error();
        fprintf(stderr, "music_player: %.*s\n", (int)error.length, error.data);
        return 1;
    }
    return 0;
}
