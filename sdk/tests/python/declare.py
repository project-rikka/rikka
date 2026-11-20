from dataclasses import dataclass, field

import rikka


@dataclass
class Notes:
    document: rikka.TextDocument = field(default_factory=lambda: rikka.TextDocument("draft"))
    picture: rikka.Image = field(default_factory=lambda: rikka.Image.from_file("/nonexistent.png"))
    clip: rikka.Video = field(default_factory=lambda: rikka.Video("/nonexistent.webm"))
    items: list[str] = field(default_factory=lambda: ["one", "two"])
    page: int = 0
    dark: bool = False


def declare(view: rikka.View, notes: Notes) -> None:
    notes.dark = view.is_dark
    accent: rikka.Color = view.palette_color(rikka.ColorRole.ACCENT)
    with view.column().padding(rikka.SPACE_LOOSE) as column:
        view.text("Notes").text_style(rikka.TextRole.TITLE).tint(accent)
        with view.row().gap(rikka.SPACE_SNUG):

            @view.button("Save").button_style(rikka.ButtonVariant.PRIMARY).on_press
            def save(context: rikka.Context) -> None:
                context.set_clipboard_text(notes.document.text)

                @context.after("autosave", 500)
                def autosave(later: rikka.Context) -> None:
                    later.skip_view()

            def toggled(context: rikka.Context, on: bool) -> None:
                notes.dark = on

            view.toggle(notes.dark).on_toggle(toggled)
            view.slider(0.5).range(0, 2).on_value(lambda context, value: None)
        with view.segmented(notes.page) as segments:
            view.segment("Edit")
            view.segment("Preview")

        def selected(context: rikka.Context, page: int) -> None:
            notes.page = page

        segments.on_select(selected)
        view.text_editor(notes.document).code(True).syntax("rust").on_change(lambda context: None)

        def dropped(context: rikka.Context, paths: list[str]) -> None:
            notes.picture = rikka.Image.from_file(paths[0])

        view.image_view(notes.picture).image_fit(rikka.Fit.CONTAIN).on_drop_files(dropped).on_drop_text(
            lambda context, text: notes.document.insert(text)
        )
        view.video_view(notes.clip).image_fit(rikka.Fit.COVER).height(120)

        @view.icon_button("play_arrow").tooltip("Play").on_press
        def play(context: rikka.Context) -> None:
            @notes.clip.on_end(context.window)
            def ended(later: rikka.Context) -> None:
                later.confirm("Again?", "The clip ended.", "Replay", lambda answered, replay: None)

            notes.clip.play()

        @view.lazy_list(len(notes.items))
        def item(row: rikka.View, index: int, chosen: bool) -> None:
            row.list_item(notes.items[index]).trailing_text("chosen" if chosen else "")

        view.memo(notes.document.revision, lambda content: content.spinner())

        def answered(context: rikka.Context, answer: rikka.UnsavedAnswer) -> None:
            if answer is rikka.UnsavedAnswer.DISCARD:
                context.request(rikka.WindowRequest.CLOSE)

        view.unsaved_prompt("draft", False, answered)

    @column.on_shortcut("s", rikka.Modifiers.CONTROL)
    def save_as(context: rikka.Context) -> None:
        def chosen(done: rikka.Context, status: rikka.Status, paths: list[str]) -> None:
            if status is rikka.Status.OK:
                done.window.set_title(paths[0])

        context.choose_files(
            rikka.FileChooser(rikka.FileChooserPurpose.SAVE, "Save", name="notes.txt"), chosen
        )
        context.run_in_background(lambda: None, lambda done: done.window.invalidate())


def window(notes: Notes) -> rikka.WindowBuilder:
    builder = rikka.WindowBuilder(lambda view: declare(view, notes), title="Notes", size=(800, 600))

    @builder.on_shape
    def shaped(
        context: rikka.Context, width: float, height: float, maximized: bool, fullscreen: bool
    ) -> None:
        context.skip_view()

    return builder


def main() -> None:
    app = rikka.App("org.rikka.notes", single_instance=True)
    app.add_window(window(Notes()))
    try:
        app.run()
    except rikka.Error as error:
        print(error.status.name, error)
