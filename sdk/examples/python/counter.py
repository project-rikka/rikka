import sys

import rikka


class Counter:
    def __init__(self) -> None:
        self.count = 0

    def view(self, view: rikka.View) -> None:
        with view.column().gap(rikka.SPACE_ROOMY).padding(rikka.SPACE_LOOSE):
            view.text(str(self.count)).text_style(rikka.TextRole.DISPLAY)
            with view.row().gap(rikka.SPACE_SNUG):
                view.button("Add").on_press(self.add)
                view.button("Remove").button_style(rikka.ButtonVariant.SECONDARY).enabled(
                    self.count > 0
                ).on_press(self.remove)

    def add(self, context: rikka.Context) -> None:
        self.count += 1

    def remove(self, context: rikka.Context) -> None:
        self.count -= 1


def main() -> int:
    counter = Counter()
    app = rikka.App("org.rikka.examples.counter")
    app.add_window(rikka.WindowBuilder(counter.view, title="Counter", size=(360, 240)))
    try:
        app.run()
    except rikka.Error as error:
        print(f"counter: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
