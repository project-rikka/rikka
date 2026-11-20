#include <rikka/rikka.hpp>

#include <array>
#include <cstdint>
#include <cstdio>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

static_assert(!std::is_copy_constructible_v<rikka::App>, "owned classes are move-only");
static_assert(std::is_nothrow_move_constructible_v<rikka::WindowBuilder>, "owned classes move");
static_assert(std::is_copy_constructible_v<rikka::Image>, "shared classes copy a reference");
static_assert(std::is_copy_constructible_v<rikka::TextDocument>, "local classes copy a reference");
static_assert(!std::is_copy_constructible_v<rikka::Context>, "scoped classes are only lent");
static_assert(((rikka::Modifiers::control | rikka::Modifiers::shift) & rikka::Modifiers::shift) ==
                  rikka::Modifiers::shift,
              "flags combine");
static_assert(rikka::KeyCode{U'a'} != rikka::KeyCode::enter, "open enums hold any value");

static int failures;

#define CHECK(condition)                                                                   \
    do {                                                                                   \
        if (!(condition)) {                                                                \
            std::fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, #condition); \
            failures++;                                                                    \
        }                                                                                  \
    } while (0)

namespace {

class Tracker {
public:
    explicit Tracker(int& destroyed) : destroyed_(&destroyed) {}

    Tracker(Tracker&& other) noexcept : destroyed_(std::exchange(other.destroyed_, nullptr)) {}

    Tracker(const Tracker&) = delete;
    Tracker& operator=(const Tracker&) = delete;
    Tracker& operator=(Tracker&&) = delete;

    ~Tracker() {
        if (destroyed_ != nullptr) {
            ++*destroyed_;
        }
    }

private:
    int* destroyed_;
};

struct Notes {
    rikka::TextDocument document{"draft"};
    rikka::Image picture = rikka::Image::from_file("/nonexistent.png");
    rikka::Video clip{"/nonexistent.webm"};
    std::vector<std::string> items{"one", "two"};
    std::size_t page = 0;
    bool dark = false;
};

void declare(rikka::View& view, Notes& notes) {
    notes.dark = view.is_dark();
    rikka::Color accent = view.palette_color(rikka::ColorRole::accent);
    view.column([&] {
            view.text("Notes").text_style(rikka::TextRole::title).tint(accent);
            view.row([&] {
                    view.button("Save")
                        .button_style(rikka::ButtonVariant::primary)
                        .on_press([&](rikka::Context& context) {
                            context.set_clipboard_text(notes.document.text());
                            context.after("autosave", 500, [](rikka::Context& later) { later.skip_view(); });
                        });
                    view.toggle(notes.dark).on_toggle([&](rikka::Context&, bool on) { notes.dark = on; });
                    view.slider(0.5).range(0, 2).on_value([](rikka::Context&, double) {});
                })
                .gap(rikka::space_snug);
            view.segmented(notes.page, [&] {
                    view.segment("Edit");
                    view.icon_segment("visibility", "Preview");
                })
                .on_select([&](rikka::Context&, std::size_t page) { notes.page = page; });
            view.text_editor(notes.document).code(true).syntax("rust").on_change([](rikka::Context&) {});
            view.image_view(notes.picture)
                .image_fit(rikka::Fit::contain)
                .on_drop_files([&](rikka::Context&, std::span<const std::string_view> paths) {
                    notes.picture = rikka::Image::from_file(paths.front());
                })
                .on_drop_text([&](rikka::Context&, std::string_view text) { notes.document.insert(text); });
            view.video_view(notes.clip).image_fit(rikka::Fit::cover).height(120);
            view.icon_button("play_arrow").tooltip("Play").on_press([&](rikka::Context& context) {
                notes.clip.on_end(context.window(), [](rikka::Context& ended) {
                    ended.confirm("Again?", "The clip ended.", "Replay", [](rikka::Context&, bool) {});
                });
                notes.clip.play();
            });
            view.lazy_list(notes.items.size(), [&](rikka::View& item, std::size_t index, bool selected) {
                    item.list_item(notes.items[index]).checked(selected);
                })
                .item_extent(32)
                .on_activate([](rikka::Context& context, std::size_t) {
                    context.read_clipboard_text([](rikka::Context&, bool, std::string_view) {});
                });
            view.memo(notes.document.revision(), [](rikka::View& content) { content.spinner(); });
            view.unsaved_prompt("draft", false, [](rikka::Context& context, rikka::UnsavedAnswer answer) {
                if (answer == rikka::UnsavedAnswer::discard) {
                    context.request(rikka::WindowRequest::close);
                }
            });
        })
        .padding(rikka::space_loose)
        .on_shortcut(rikka::KeyCode{U's'}, rikka::Modifiers::control, [](rikka::Context& context) {
            context.choose_files(rikka::FileChooser(rikka::FileChooserPurpose::save, "Save").name("notes.txt"),
                                 [](rikka::Context&, rikka::Status, std::span<const std::string_view>) {});
            context.run_in_background([] {}, [](rikka::Context& done) { done.window().set_title("Saved"); });
        });
}

void version_matches_header() {
    CHECK(rikka::version() == rikka::header_version);
    CHECK(rikka::last_error().empty());
}

void text_round_trips_utf8() {
    constexpr std::string_view prefix = "日本語 text";
    rikka::TextDocument document(std::string_view("日本語 text, not terminated here").substr(0, prefix.size()));
    CHECK(document.text() == prefix);
    CHECK(document.length() == prefix.size());

    document.set_text("terminated ✓");
    CHECK(document.text() == "terminated ✓");

    document.set_text({});
    CHECK(document.length() == 0);
    CHECK(document.text().empty());
}

void document_copies_share_edits_and_undo() {
    rikka::TextDocument document("first");
    CHECK(!document.is_modified());
    CHECK(!document.can_undo());

    std::uint64_t before = document.revision();
    rikka::TextDocument kept = document;
    kept.set_text("second");
    CHECK(document.revision() != before);
    CHECK(document.is_modified());
    CHECK(document.text() == "second");

    CHECK(document.undo());
    CHECK(kept.text() == "first");
    CHECK(document.redo());
    document.mark_saved();
    CHECK(!kept.is_modified());

    document.select_all();
    document.insert("new");
    CHECK(kept.text() == "new");
    CHECK(kept.selected_text().empty());

    document.reset("fresh");
    CHECK(!document.can_undo());
    CHECK(!document.undo());
}

void builders_release_their_closures() {
    int destroyed = 0;
    {
        rikka::WindowBuilder alone([tracker = Tracker(destroyed)](rikka::View&) {});
        alone.title("Alone")
            .on_start([tracker = Tracker(destroyed)](rikka::Context&) {})
            .add_shortcut(rikka::KeyCode{U's'}, rikka::Modifiers::control,
                          [tracker = Tracker(destroyed)](rikka::Context&) {});
        CHECK(destroyed == 0);
    }
    CHECK(destroyed == 3);

    {
        rikka::App app("org.rikka.sdk.test");
        app.add_window(rikka::WindowBuilder([tracker = Tracker(destroyed)](rikka::View&) {}).size(320, 200))
            .add_window(rikka::WindowBuilder([tracker = Tracker(destroyed)](rikka::View&) {}));
        rikka::App moved = std::move(app);
        CHECK(app.raw() == nullptr);
        CHECK(destroyed == 3);
    }
    CHECK(destroyed == 5);
}

void images_share_one_reference() {
    std::array<std::uint8_t, 2 * 3 * 4> pixels{};
    pixels.fill(0x7f);
    rikka::Image rgba = rikka::Image::from_rgba(2, 3, pixels);
    rikka::Image kept = rgba;
    CHECK(kept.raw() == rgba.raw());
    rikka::Image taken = std::move(rgba);
    CHECK(rgba.raw() == nullptr);
    CHECK(taken.raw() == kept.raw());

    const std::array<std::uint8_t, 4> encoded{0x89, 'P', 'N', 'G'};
    rikka::Image png = rikka::Image::from_encoded(encoded);
    CHECK(png.raw() != nullptr);
}

void missing_sounds_fail_and_keep_settings() {
    rikka::Sound sound("/nonexistent.ogg");
    CHECK(sound.state() == rikka::PlaybackState::failed);
    CHECK(sound.error_message().find("/nonexistent.ogg") != std::string::npos);
    sound.set_volume(0.5);
    sound.set_looping(true);
    CHECK(sound.volume() == 0.5);
    CHECK(sound.looping());
    rikka::Sound kept = sound;
    CHECK(kept.raw() == sound.raw());
}

void opening_a_relative_url_throws() {
    try {
        rikka::open_url("example.org");
        CHECK(false);
    } catch (const rikka::Error& error) {
        CHECK(error.status() == rikka::Status::failed);
        CHECK(std::string_view(error.what()).find("example.org") != std::string_view::npos);
    }
}

void running_without_a_display_throws() {
    int destroyed = 0;
    Notes notes;
    try {
        rikka::App("org.rikka.sdk.test")
            .add_window(rikka::WindowBuilder([&notes, tracker = Tracker(destroyed)](rikka::View& view) {
                            declare(view, notes);
                        })
                            .title("Never shown"))
            .run();
        CHECK(false);
    } catch (const rikka::Error& error) {
        CHECK(error.status() == rikka::Status::no_display);
        CHECK(!std::string_view(error.what()).empty());
    }
    CHECK(destroyed == 1);
}

}  // namespace

int main() {
    version_matches_header();
    text_round_trips_utf8();
    document_copies_share_edits_and_undo();
    builders_release_their_closures();
    images_share_one_reference();
    missing_sounds_fail_and_keep_settings();
    opening_a_relative_url_throws();
    running_without_a_display_throws();
    if (failures > 0) {
        std::fprintf(stderr, "%d checks failed\n", failures);
        return 1;
    }
    return 0;
}
