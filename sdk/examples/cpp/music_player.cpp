#include <rikka/rikka.hpp>

#include <algorithm>
#include <cstdio>
#include <filesystem>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <vector>

namespace {

std::string clock(double seconds) {
    int whole = static_cast<int>(seconds);
    int rest = whole % 60;
    return std::to_string(whole / 60) + (rest < 10 ? ":0" : ":") + std::to_string(rest);
}

class MusicPlayer {
public:
    void view(rikka::View& view) {
        view.column([&] {
                view.row([&] {
                        view.text("Music").text_style(rikka::TextRole::title).grow(1);
                        view.button("Choose Music…").on_press([this](rikka::Context& context) { choose(context); });
                    })
                    .gap(rikka::space_snug)
                    .align_items(rikka::Align::center);
                if (tracks_.empty()) {
                    view.text("Choose one or more music files to play them in order.").tone(rikka::TextTone::muted);
                    return;
                }
                view.list([&] {
                        for (const std::string& track : tracks_) {
                            view.list_item(std::filesystem::path(track).stem().string()).leading_icon("music_note");
                        }
                    })
                    .selection(current_)
                    .on_select([this](rikka::Context& context, std::size_t index) { start(context, index); })
                    .grow(1);
                if (sound_) {
                    controls(view, *sound_);
                }
            })
            .gap(rikka::space_roomy)
            .padding(rikka::space_loose)
            .fill_width()
            .fill_height();
    }

private:
    void controls(rikka::View& view, rikka::Sound sound) {
        if (sound.state() == rikka::PlaybackState::failed) {
            view.banner(sound.error_message());
            return;
        }
        double duration = sound.duration();
        double position = scrub_.value_or(sound.position());
        view.slider(position)
            .range(0, std::max(duration, 1.0))
            .enabled(duration > 0)
            .on_value([this](rikka::Context&, double value) { scrub_ = value; })
            .on_value_commit([this, sound](rikka::Context&, double value) mutable {
                scrub_.reset();
                sound.seek(value);
            });
        view.row([&] {
                view.icon_button("skip_previous")
                    .tooltip("Previous")
                    .enabled(current_ > 0)
                    .on_press([this](rikka::Context& context) { start(context, current_ - 1); });
                view.icon_button(sound.is_playing() ? "pause" : "play_arrow")
                    .tooltip(sound.is_playing() ? "Pause" : "Play")
                    .on_press([sound](rikka::Context&) mutable {
                        if (sound.is_playing()) {
                            sound.pause();
                        } else {
                            sound.play();
                        }
                    });
                view.icon_button("skip_next")
                    .tooltip("Next")
                    .enabled(current_ + 1 < tracks_.size())
                    .on_press([this](rikka::Context& context) { next(context); });
                view.text(clock(position) + " / " + clock(duration)).text_monospace();
                view.spacer();
                view.checkbox(sound.looping() ? rikka::CheckState::checked : rikka::CheckState::unchecked, "Repeat")
                    .on_toggle([sound](rikka::Context&, bool on) mutable { sound.set_looping(on); });
                view.icon("volume_up");
                view.slider(sound.volume()).width(120).on_value([sound](rikka::Context&, double value) mutable {
                    sound.set_volume(value);
                });
            })
            .gap(rikka::space_snug)
            .align_items(rikka::Align::center);
    }

    void choose(rikka::Context& context) {
        auto chooser = rikka::FileChooser(rikka::FileChooserPurpose::open, "Choose Music")
                           .multiple(true)
                           .add_filter("Music")
                           .add_filter_mime_type("audio/*");
        context.choose_files(std::move(chooser),
                             [this](rikka::Context& chosen, rikka::Status status, std::span<const std::string_view> paths) {
                                 if (status != rikka::Status::ok) {
                                     return;
                                 }
                                 tracks_.assign(paths.begin(), paths.end());
                                 start(chosen, 0);
                             });
    }

    void start(rikka::Context& context, std::size_t index) {
        if (sound_) {
            sound_->stop();
        }
        current_ = index;
        scrub_.reset();
        sound_.emplace(tracks_[index]);
        sound_->on_end(context.window(), [this](rikka::Context& ended) { next(ended); });
        sound_->play();
    }

    void next(rikka::Context& context) {
        if (current_ + 1 < tracks_.size()) {
            start(context, current_ + 1);
        }
    }

    std::vector<std::string> tracks_;
    std::size_t current_ = 0;
    std::optional<rikka::Sound> sound_;
    std::optional<double> scrub_;
};

}

int main() {
    MusicPlayer player;
    rikka::WindowBuilder window([&player](rikka::View& view) { player.view(view); });
    window.title("Music Player").size(560, 520);

    try {
        rikka::App("org.rikka.examples.music_player").add_window(std::move(window)).run();
    } catch (const rikka::Error& error) {
        std::fprintf(stderr, "music_player: %s\n", error.what());
        return 1;
    }
    return 0;
}
