#include <rikka/rikka.hpp>

#include <cstdio>
#include <string>
#include <string_view>
#include <utility>

constexpr std::string_view website = "https://github.com/project-rikka/rikka";

int main() {
    std::string answer = "Nothing was deleted yet.";

    rikka::WindowBuilder window([&answer](rikka::View& view) {
        view.column([&] {
                view.text("Dialogs").text_style(rikka::TextRole::title);
                view.button("Show a Message").on_press([](rikka::Context& context) {
                    context.alert("Hello", "This message was shown with Context::alert.");
                });
                view.row([&] {
                        view.button("Delete Everything…")
                            .button_style(rikka::ButtonVariant::danger)
                            .on_press([&answer](rikka::Context& context) {
                                context.confirm("Delete everything?", "This cannot be undone.", "Delete",
                                                [&answer](rikka::Context&, bool confirmed) {
                                                    answer = confirmed ? "Everything was deleted."
                                                                       : "You kept everything.";
                                                });
                            });
                        view.text(answer).tone(rikka::TextTone::muted);
                    })
                    .gap(rikka::space_normal)
                    .align_items(rikka::Align::center);
                view.button("Visit the Website")
                    .button_style(rikka::ButtonVariant::secondary)
                    .on_press([](rikka::Context& context) {
                        try {
                            rikka::open_url(website);
                        } catch (const rikka::Error& error) {
                            context.alert("Couldn’t Open the Website", error.what());
                        }
                    });
            })
            .gap(rikka::space_roomy)
            .padding(rikka::space_loose);
    });
    window.title("Dialogs").size(480, 320);

    try {
        rikka::App("org.rikka.examples.dialogs").add_window(std::move(window)).run();
    } catch (const rikka::Error& error) {
        std::fprintf(stderr, "dialogs: %s\n", error.what());
        return 1;
    }
    return 0;
}
