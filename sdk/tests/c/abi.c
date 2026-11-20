#define _POSIX_C_SOURCE 200809L

#include <rikka/rikka.h>

#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

static int failures;

#define CHECK(condition)                                                                   \
    do {                                                                                   \
        if (!(condition)) {                                                                \
            fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, #condition); \
            failures++;                                                                    \
        }                                                                                  \
    } while (0)

static bool owned(rk_string string, const char *expected) {
    bool equal = string.length == strlen(expected) && memcmp(string.data, expected, string.length) == 0 &&
                 string.data[string.length] == '\0';
    rk_string_free(string);
    return equal;
}

static void count_destroy(void *userdata) {
    (*(int *)userdata)++;
}

static void declare_nothing(rk_view *view, void *userdata) {
    (void)view;
    (void)userdata;
}

static void ignore(rk_context *context, void *userdata) {
    (void)context;
    (void)userdata;
}

static void version_matches_header(void) {
    CHECK(rk_version() == RK_HEADER_VERSION);
    CHECK(rk_last_error().length == 0);
}

static void text_round_trips_utf8(void) {
    const char bytes[] = "日本語 text, not terminated here";
    rk_str prefix = {bytes, strlen("日本語 text")};
    rk_text_document *document = rk_text_document_new(prefix);
    CHECK(owned(rk_text_document_text(document), "日本語 text"));
    CHECK(rk_text_document_length(document) == strlen("日本語 text"));

    rk_text_document_set_text(document, rk_cstr("terminated ✓"));
    CHECK(owned(rk_text_document_text(document), "terminated ✓"));

    rk_str empty = {NULL, 0};
    rk_text_document_set_text(document, empty);
    CHECK(rk_text_document_length(document) == 0);
    rk_text_document_unref(document);
}

static void document_tracks_edits_and_undo(void) {
    rk_text_document *document = rk_text_document_new(RK_STR("first"));
    CHECK(!rk_text_document_is_modified(document));
    CHECK(!rk_text_document_can_undo(document));

    uint64_t before = rk_text_document_revision(document);
    rk_text_document_set_text(document, RK_STR("second"));
    CHECK(rk_text_document_revision(document) != before);
    CHECK(rk_text_document_is_modified(document));
    CHECK(rk_text_document_can_undo(document));

    CHECK(rk_text_document_undo(document));
    CHECK(owned(rk_text_document_text(document), "first"));
    CHECK(rk_text_document_can_redo(document));
    CHECK(rk_text_document_redo(document));
    CHECK(owned(rk_text_document_text(document), "second"));

    rk_text_document_mark_saved(document);
    CHECK(!rk_text_document_is_modified(document));

    rk_text_document_reset(document, RK_STR("fresh"));
    CHECK(!rk_text_document_can_undo(document));
    CHECK(!rk_text_document_undo(document));
    CHECK(owned(rk_text_document_text(document), "fresh"));

    rk_text_document *kept = rk_text_document_ref(document);
    CHECK(kept == document);
    rk_text_document_unref(document);
    CHECK(owned(rk_text_document_text(kept), "fresh"));
    rk_text_document_unref(kept);
}

static void document_insert_replaces_selection(void) {
    rk_text_document *document = rk_text_document_new(RK_STR("old"));
    rk_text_document_select_all(document);
    CHECK(owned(rk_text_document_selected_text(document), "old"));
    rk_text_document_insert(document, RK_STR("new"));
    CHECK(owned(rk_text_document_text(document), "new"));
    CHECK(owned(rk_text_document_selected_text(document), ""));
    rk_text_document_unref(document);
}

static void releasing_null_does_nothing(void) {
    rk_string nothing = {NULL, 0};
    rk_string_free(nothing);
    rk_app_free(NULL);
    rk_window_builder_free(NULL);
    rk_file_chooser_free(NULL);
    rk_window_unref(NULL);
    rk_image_unref(NULL);
    rk_sound_unref(NULL);
    rk_video_unref(NULL);
    rk_text_document_unref(NULL);
    CHECK(rk_image_ref(NULL) == NULL);
}

static void builders_release_their_closures(void) {
    int destroyed = 0;
    rk_window_builder *alone = rk_window_builder_new(declare_nothing, &destroyed, count_destroy);
    rk_window_builder_on_start(alone, ignore, &destroyed, count_destroy);
    rk_window_builder_add_shortcut(alone, 's', RK_MODIFIER_CONTROL, ignore, &destroyed, count_destroy);
    CHECK(destroyed == 0);
    rk_window_builder_free(alone);
    CHECK(destroyed == 3);

    rk_app *app = rk_app_new(RK_STR("org.rikka.sdk.test"));
    rk_app_add_window(app, rk_window_builder_new(declare_nothing, &destroyed, count_destroy));
    rk_app_add_window(app, rk_window_builder_new(declare_nothing, &destroyed, count_destroy));
    CHECK(destroyed == 3);
    rk_app_free(app);
    CHECK(destroyed == 5);
}

static void running_without_a_display_fails_cleanly(void) {
    int destroyed = 0;
    rk_app *app = rk_app_new(RK_STR("org.rikka.sdk.test"));
    rk_window_builder *builder = rk_window_builder_new(declare_nothing, &destroyed, count_destroy);
    rk_window_builder_set_title(builder, RK_STR("Never shown"));
    rk_app_add_window(app, builder);
    rk_status status = rk_app_run(app);
    CHECK(status == RK_STATUS_NO_DISPLAY);
    CHECK(rk_last_error().length > 0);
    CHECK(destroyed == 1);
}

static void images_copy_their_input(void) {
    uint8_t pixels[2 * 3 * 4];
    memset(pixels, 0x7f, sizeof pixels);
    rk_image *rgba = rk_image_new_rgba(2, 3, pixels, sizeof pixels);
    memset(pixels, 0, sizeof pixels);
    rk_image *kept = rk_image_ref(rgba);
    CHECK(kept == rgba);
    rk_image_unref(rgba);
    rk_image_unref(kept);

    const uint8_t encoded[] = {0x89, 'P', 'N', 'G'};
    rk_image_unref(rk_image_new_encoded(encoded, sizeof encoded));
    rk_image_unref(rk_image_new_file(RK_STR("/nonexistent.png")));
}

static void file_choosers_collect_filters(void) {
    rk_file_chooser *chooser = rk_file_chooser_new(RK_FILE_CHOOSER_PURPOSE_OPEN, RK_STR("Open"));
    rk_file_chooser_set_multiple(chooser, true);
    rk_file_chooser_add_filter(chooser, RK_STR("Pictures"));
    rk_file_chooser_add_filter_pattern(chooser, RK_STR("*.png"));
    rk_file_chooser_add_filter_mime_type(chooser, RK_STR("image/jpeg"));
    rk_file_chooser_free(chooser);
}

static bool mentions(rk_string string, const char *expected) {
    bool found = string.length > 0 && strstr(string.data, expected) != NULL;
    rk_string_free(string);
    return found;
}

static bool last_error_mentions(const char *expected) {
    rk_str error = rk_last_error();
    char copy[512] = {0};
    memcpy(copy, error.data, error.length < sizeof copy - 1 ? error.length : sizeof copy - 1);
    return strstr(copy, expected) != NULL;
}

static void missing_media_fails_at_once(void) {
    rk_sound *sound = rk_sound_new(RK_STR("/nonexistent.ogg"));
    CHECK(rk_sound_state(sound) == RK_PLAYBACK_STATE_FAILED);
    CHECK(mentions(rk_sound_error_message(sound), "/nonexistent.ogg"));
    rk_sound_play(sound);
    CHECK(!rk_sound_is_playing(sound));
    CHECK(rk_sound_state(sound) == RK_PLAYBACK_STATE_FAILED);
    CHECK(rk_sound_position(sound) == 0);
    CHECK(rk_sound_duration(sound) == 0);
    rk_sound_unref(sound);

    rk_video *video = rk_video_new(RK_STR("/nonexistent.webm"));
    CHECK(rk_video_state(video) == RK_PLAYBACK_STATE_FAILED);
    CHECK(mentions(rk_video_error_message(video), "/nonexistent.webm"));
    rk_video_play(video);
    CHECK(!rk_video_is_playing(video));
    CHECK(rk_video_width(video) == 0 && rk_video_height(video) == 0);
    rk_video_unref(video);
}

static void media_settings_round_trip(void) {
    rk_sound *sound = rk_sound_new(RK_STR("/nonexistent.ogg"));
    CHECK(rk_sound_volume(sound) == 1);
    CHECK(!rk_sound_looping(sound));
    rk_sound_set_volume(sound, 0.25);
    rk_sound_set_looping(sound, true);
    CHECK(rk_sound_volume(sound) == 0.25);
    CHECK(rk_sound_looping(sound));
    rk_sound *kept = rk_sound_ref(sound);
    CHECK(kept == sound);
    rk_sound_unref(sound);
    rk_sound_unref(kept);

    rk_video *video = rk_video_new(RK_STR("/nonexistent.webm"));
    rk_video_set_volume(video, 0);
    CHECK(rk_video_volume(video) == 0);
    rk_video_unref(video);
}

static void opening_rejects_what_cannot_be_opened(void) {
    CHECK(rk_open_url(RK_STR("example.org")) == RK_STATUS_FAILED);
    CHECK(last_error_mentions("`example.org` is not an absolute URL"));
    CHECK(rk_open_url(RK_STR("https://example.org/a b")) == RK_STATUS_FAILED);
    CHECK(rk_open_file(RK_STR("/nonexistent/picture.png")) == RK_STATUS_FAILED);
    CHECK(last_error_mentions("/nonexistent/picture.png"));
}

typedef void (*violation_fn)(void);

static void invalid_utf8(void) {
    const char bytes[] = {(char)0xff, (char)0xfe};
    rk_str text = {bytes, sizeof bytes};
    rk_text_document_unref(rk_text_document_new(text));
}

static void null_handle(void) {
    rk_text_document_length(NULL);
}

static void unknown_enum(void) {
    rk_file_chooser_free(rk_file_chooser_new((rk_file_chooser_purpose)99, RK_STR("Open")));
}

static void short_pixels(void) {
    const uint8_t pixels[4] = {0};
    rk_image_unref(rk_image_new_rgba(2, 2, pixels, sizeof pixels));
}

static void bad_application_id(void) {
    rk_app_free(rk_app_new(RK_STR("not an id")));
}

static void filter_pattern_without_filter(void) {
    rk_file_chooser *chooser = rk_file_chooser_new(RK_FILE_CHOOSER_PURPOSE_SAVE, RK_STR("Save"));
    rk_file_chooser_add_filter_pattern(chooser, RK_STR("*.txt"));
}

static void *read_length(void *document) {
    rk_text_document_length(document);
    return NULL;
}

static void document_on_another_thread(void) {
    rk_text_document *document = rk_text_document_new(RK_STR("local"));
    pthread_t thread;
    pthread_create(&thread, NULL, read_length, document);
    pthread_join(thread, NULL);
}

static void loud_volume(void) {
    rk_sound_set_volume(rk_sound_new(RK_STR("/nonexistent.ogg")), 2);
}

static void negative_seek(void) {
    rk_video_seek(rk_video_new(RK_STR("/nonexistent.webm")), -1);
}

struct violation {
    const char *name;
    violation_fn run;
    const char *diagnostic;
};

static const struct violation VIOLATIONS[] = {
    {"invalid UTF-8", invalid_utf8, "rikka: rk_text_document_new: `text` is not UTF-8"},
    {"NULL handle", null_handle, "rikka: rk_text_document_length: `document` is NULL"},
    {"unknown enum", unknown_enum, "rikka: rk_file_chooser_new: `purpose` is not a valid rk_file_chooser_purpose: 99"},
    {"short pixels", short_pixels, "rikka: rk_image_new_rgba: 4 bytes are not 2 x 2 RGBA8 pixels"},
    {"bad application id", bad_application_id, "rikka: rk_app_new: `not an id` is not a reverse-DNS application id"},
    {"pattern without filter", filter_pattern_without_filter, "rikka: rk_file_chooser_add_filter_pattern: no filter was added"},
    {"another thread", document_on_another_thread, "rikka: rk_text_document_length: `document` belongs to another thread"},
    {"loud volume", loud_volume, "rikka: rk_sound_set_volume: the volume 2 is not between 0 and 1"},
    {"negative seek", negative_seek, "rikka: rk_video_seek: the position -1 is not a finite number of seconds of at least 0"},
};

static void contract_violations_abort_with_a_diagnostic(void) {
    for (size_t index = 0; index < sizeof VIOLATIONS / sizeof VIOLATIONS[0]; index++) {
        const struct violation *violation = &VIOLATIONS[index];
        int pipe_ends[2];
        CHECK(pipe(pipe_ends) == 0);
        fflush(stderr);
        pid_t child = fork();
        if (child == 0) {
            dup2(pipe_ends[1], STDERR_FILENO);
            close(pipe_ends[0]);
            violation->run();
            _exit(0);
        }
        close(pipe_ends[1]);
        char printed[1024] = {0};
        size_t length = 0;
        ssize_t read_now;
        while (length < sizeof printed - 1 &&
               (read_now = read(pipe_ends[0], printed + length, sizeof printed - 1 - length)) > 0) {
            length += (size_t)read_now;
        }
        close(pipe_ends[0]);
        int status = 0;
        waitpid(child, &status, 0);
        bool aborted = WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT;
        bool named = strstr(printed, violation->diagnostic) != NULL;
        if (!aborted || !named) {
            fprintf(stderr, "%s: aborted %d, printed: %s\n", violation->name, aborted, printed);
        }
        CHECK(aborted);
        CHECK(named);
    }
}

int main(void) {
    version_matches_header();
    text_round_trips_utf8();
    document_tracks_edits_and_undo();
    document_insert_replaces_selection();
    releasing_null_does_nothing();
    builders_release_their_closures();
    images_copy_their_input();
    file_choosers_collect_filters();
    missing_media_fails_at_once();
    media_settings_round_trip();
    opening_rejects_what_cannot_be_opened();
    contract_violations_abort_with_a_diagnostic();
    running_without_a_display_fails_cleanly();
    if (failures > 0) {
        fprintf(stderr, "%d checks failed\n", failures);
        return 1;
    }
    return 0;
}
