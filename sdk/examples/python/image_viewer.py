import sys
from pathlib import Path

import rikka


class ImageViewer:
    def __init__(self) -> None:
        self.path: str | None = None
        self.image: rikka.Image | None = None

    def view(self, view: rikka.View) -> None:
        with view.column().fill_width().fill_height().on_drop_files(self.dropped):
            with view.row().gap(rikka.SPACE_SNUG).padding(rikka.SPACE_NORMAL).align_items(
                rikka.Align.CENTER
            ):
                view.button("Open Picture…").on_press(self.choose)
                view.icon_button("open_in_new").tooltip("Open in Default App").enabled(
                    self.path is not None
                ).on_press(self.open_elsewhere)
                view.text(Path(self.path).name if self.path else "").text_no_wrap().grow(1)
            view.divider()
            if self.image is None:
                view.status_page("image", "Drop a picture here, or open one.").grow(1)
            else:
                view.image_view(self.image).image_fit(rikka.Fit.CONTAIN).fill_width().grow(1)

    def show(self, path: str) -> None:
        self.path = path
        self.image = rikka.Image.from_file(path)

    def choose(self, context: rikka.Context) -> None:
        chooser = rikka.FileChooser(rikka.FileChooserPurpose.OPEN, "Open Picture")
        chooser.add_filter("Pictures")
        chooser.add_filter_mime_type("image/*")
        context.choose_files(chooser, self.chosen)

    def chosen(self, context: rikka.Context, status: rikka.Status, paths: list[str]) -> None:
        if status == rikka.Status.OK:
            self.show(paths[0])

    def dropped(self, context: rikka.Context, paths: list[str]) -> None:
        self.show(paths[0])

    def open_elsewhere(self, context: rikka.Context) -> None:
        if self.path is None:
            return
        try:
            rikka.open_file(self.path)
        except rikka.Error as error:
            context.alert("Couldn’t Open the Picture", str(error))


def main() -> int:
    viewer = ImageViewer()
    app = rikka.App("org.rikka.examples.image_viewer")
    app.add_window(rikka.WindowBuilder(viewer.view, title="Image Viewer", size=(720, 540)))
    try:
        app.run()
    except rikka.Error as error:
        print(f"image_viewer: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
