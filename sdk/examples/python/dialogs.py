import sys

import rikka

WEBSITE = "https://github.com/project-rikka/rikka"


class Dialogs:
    def __init__(self) -> None:
        self.answer = "Nothing was deleted yet."

    def view(self, view: rikka.View) -> None:
        with view.column().gap(rikka.SPACE_ROOMY).padding(rikka.SPACE_LOOSE):
            view.text("Dialogs").text_style(rikka.TextRole.TITLE)
            view.button("Show a Message").on_press(self.show_message)
            with view.row().gap(rikka.SPACE_NORMAL).align_items(rikka.Align.CENTER):
                view.button("Delete Everything…").button_style(rikka.ButtonVariant.DANGER).on_press(
                    self.ask_to_delete
                )
                view.text(self.answer).tone(rikka.TextTone.MUTED)
            view.button("Visit the Website").button_style(rikka.ButtonVariant.SECONDARY).on_press(
                self.visit
            )

    def show_message(self, context: rikka.Context) -> None:
        context.alert("Hello", "This message was shown with context.alert.")

    def ask_to_delete(self, context: rikka.Context) -> None:
        context.confirm(
            "Delete everything?",
            "This cannot be undone.",
            "Delete",
            self.answered,
        )

    def answered(self, context: rikka.Context, confirmed: bool) -> None:
        self.answer = "Everything was deleted." if confirmed else "You kept everything."

    def visit(self, context: rikka.Context) -> None:
        try:
            rikka.open_url(WEBSITE)
        except rikka.Error as error:
            context.alert("Couldn’t Open the Website", str(error))


def main() -> int:
    dialogs = Dialogs()
    app = rikka.App("org.rikka.examples.dialogs")
    app.add_window(rikka.WindowBuilder(dialogs.view, title="Dialogs", size=(480, 320)))
    try:
        app.run()
    except rikka.Error as error:
        print(f"dialogs: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
