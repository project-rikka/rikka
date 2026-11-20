#include <rikka/rikka.hpp>

#include <algorithm>
#include <cstdio>
#include <optional>
#include <span>
#include <string>
#include <string_view>

namespace {

constexpr rikka::Color black{0, 0, 0, 1};

std::string clock(double seconds) {
    int whole = static_cast<int>(seconds);
    int rest = whole % 60;
    return std::to_string(whole / 60) + (rest < 10 ? ":0" : ":") + std::to_string(rest);
}

class VideoPlayer {
public:
    void view(rikka::View& view) {
        if (!video_) {
            view.status_page("movie", "Open a video to play it.", [&] {
                    view.button("Open Video…").on_press([this](rikka::Context& context) { choose(context); });
                })
                .fill_width()
                .fill_height();
            return;
        }
        rikka::Video video = *video_;
        view.column([&] {
                view.video_view(video).image_fit(rikka::Fit::contain).background(black).fill_width().grow(1);
                view.column([&] {
                        std::string message = video.error_message();
                        if (!message.empty()) {
                            view.banner(message);
                        }
                        controls(view, video);
                    })
                    .gap(rikka::space_snug)
                    .padding(rikka::space_normal);
            })
            .fill_width()
            .fill_height();
    }

private:
    void controls(rikka::View& view, rikka::Video video) {
        double duration = video.duration();
        double position = scrub_.value_or(video.position());
        view.slider(position)
            .range(0, std::max(duration, 1.0))
            .enabled(duration > 0)
            .on_value([this](rikka::Context&, double value) { scrub_ = value; })
            .on_value_commit([this, video](rikka::Context&, double value) mutable {
                scrub_.reset();
                video.seek(value);
            });
        view.row([&] {
                view.icon_button(video.is_playing() ? "pause" : "play_arrow")
                    .tooltip(video.is_playing() ? "Pause" : "Play")
                    .enabled(video.state() != rikka::PlaybackState::failed)
                    .on_press([video](rikka::Context&) mutable {
                        if (video.is_playing()) {
                            video.pause();
                        } else {
                            video.play();
                        }
                    });
                view.icon_button("stop").tooltip("Stop").on_press([video](rikka::Context&) mutable { video.stop(); });
                view.text(clock(position) + " / " + clock(duration)).text_monospace();
                view.spacer();
                view.icon("volume_up");
                view.slider(video.volume()).width(120).on_value([video](rikka::Context&, double value) mutable {
                    video.set_volume(value);
                });
                view.button("Open Video…")
                    .button_style(rikka::ButtonVariant::secondary)
                    .on_press([this](rikka::Context& context) { choose(context); });
            })
            .gap(rikka::space_snug)
            .align_items(rikka::Align::center);
    }

    void choose(rikka::Context& context) {
        auto chooser = rikka::FileChooser(rikka::FileChooserPurpose::open, "Open Video")
                           .add_filter("Videos")
                           .add_filter_mime_type("video/*");
        context.choose_files(std::move(chooser),
                             [this](rikka::Context&, rikka::Status status, std::span<const std::string_view> paths) {
                                 if (status != rikka::Status::ok) {
                                     return;
                                 }
                                 if (video_) {
                                     video_->stop();
                                 }
                                 scrub_.reset();
                                 video_.emplace(paths.front());
                                 video_->play();
                             });
    }

    std::optional<rikka::Video> video_;
    std::optional<double> scrub_;
};

}

int main() {
    VideoPlayer player;
    rikka::WindowBuilder window([&player](rikka::View& view) { player.view(view); });
    window.title("Video Player").size(800, 560);

    try {
        rikka::App("org.rikka.examples.video_player").add_window(std::move(window)).run();
    } catch (const rikka::Error& error) {
        std::fprintf(stderr, "video_player: %s\n", error.what());
        return 1;
    }
    return 0;
}
