#include <rikka/rikka.hpp>

#include <cstdio>
#include <filesystem>
#include <optional>
#include <span>
#include <string>
#include <string_view>

namespace {

class ImageViewer {
public:
    void view(rikka::View& view) {
        view.column([&] {
                view.row([&] {
                        view.button("Open Picture…").on_press([this](rikka::Context& context) { choose(context); });
                        view.icon_button("open_in_new")
                            .tooltip("Open in Default App")
                            .enabled(!path_.empty())
                            .on_press([this](rikka::Context& context) { open_elsewhere(context); });
                        view.text(std::filesystem::path(path_).filename().string()).text_no_wrap().grow(1);
                    })
                    .gap(rikka::space_snug)
                    .padding(rikka::space_normal)
                    .align_items(rikka::Align::center);
                view.divider();
                if (image_) {
                    view.image_view(*image_).image_fit(rikka::Fit::contain).fill_width().grow(1);
                } else {
                    view.status_page("image", "Drop a picture here, or open one.", [] {}).grow(1);
                }
            })
            .fill_width()
            .fill_height()
            .on_drop_files([this](rikka::Context&, std::span<const std::string_view> paths) { show(paths.front()); });
    }

private:
    void show(std::string_view path) {
        path_ = path;
        image_ = rikka::Image::from_file(path);
    }

    void choose(rikka::Context& context) {
        auto chooser = rikka::FileChooser(rikka::FileChooserPurpose::open, "Open Picture")
                           .add_filter("Pictures")
                           .add_filter_mime_type("image/*");
        context.choose_files(std::move(chooser),
                             [this](rikka::Context&, rikka::Status status, std::span<const std::string_view> paths) {
                                 if (status == rikka::Status::ok) {
                                     show(paths.front());
                                 }
                             });
    }

    void open_elsewhere(rikka::Context& context) {
        try {
            rikka::open_file(path_);
        } catch (const rikka::Error& error) {
            context.alert("Couldn’t Open the Picture", error.what());
        }
    }

    std::string path_;
    std::optional<rikka::Image> image_;
};

}

int main() {
    ImageViewer viewer;
    rikka::WindowBuilder window([&viewer](rikka::View& view) { viewer.view(view); });
    window.title("Image Viewer").size(720, 540);

    try {
        rikka::App("org.rikka.examples.image_viewer").add_window(std::move(window)).run();
    } catch (const rikka::Error& error) {
        std::fprintf(stderr, "image_viewer: %s\n", error.what());
        return 1;
    }
    return 0;
}
