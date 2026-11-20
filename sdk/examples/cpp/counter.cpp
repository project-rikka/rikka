#include <rikka/rikka.hpp>

#include <cstdio>
#include <string>
#include <utility>

int main() {
    long long count = 0;
    rikka::WindowBuilder window([&count](rikka::View& view) {
        view.column([&] {
                view.text(std::to_string(count)).text_style(rikka::TextRole::display);
                view.row([&] {
                        view.button("Add").on_press([&count](rikka::Context&) { ++count; });
                        view.button("Remove")
                            .button_style(rikka::ButtonVariant::secondary)
                            .enabled(count > 0)
                            .on_press([&count](rikka::Context&) { --count; });
                    })
                    .gap(rikka::space_snug);
            })
            .gap(rikka::space_roomy)
            .padding(rikka::space_loose);
    });
    window.title("Counter").size(360, 240);

    try {
        rikka::App("org.rikka.examples.counter").add_window(std::move(window)).run();
    } catch (const rikka::Error& error) {
        std::fprintf(stderr, "counter: %s\n", error.what());
        return 1;
    }
    return 0;
}
