import sys
from pathlib import Path

import rikka


def clock(seconds: float) -> str:
    whole = int(seconds)
    return f"{whole // 60}:{whole % 60:02d}"


class MusicPlayer:
    def __init__(self) -> None:
        self.tracks: list[str] = []
        self.current = 0
        self.sound: rikka.Sound | None = None
        self.scrub: float | None = None

    def view(self, view: rikka.View) -> None:
        with view.column().gap(rikka.SPACE_ROOMY).padding(
            rikka.SPACE_LOOSE
        ).fill_width().fill_height():
            with view.row().gap(rikka.SPACE_SNUG).align_items(rikka.Align.CENTER):
                view.text("Music").text_style(rikka.TextRole.TITLE).grow(1)
                view.button("Choose Music…").on_press(self.choose)
            if not self.tracks:
                view.text("Choose one or more music files to play them in order.").tone(
                    rikka.TextTone.MUTED
                )
                return
            with view.list().selection(self.current).on_select(self.pick).grow(1):
                for track in self.tracks:
                    view.list_item(Path(track).stem).leading_icon("music_note")
            if self.sound is not None:
                self.controls(view, self.sound)

    def controls(self, view: rikka.View, sound: rikka.Sound) -> None:
        if sound.state == rikka.PlaybackState.FAILED:
            view.banner(sound.error_message)
            return
        duration = sound.duration
        position = sound.position if self.scrub is None else self.scrub
        view.slider(position).range(0, max(duration, 1)).enabled(duration > 0).on_value(
            self.scrubbing
        ).on_value_commit(self.seek)
        with view.row().gap(rikka.SPACE_SNUG).align_items(rikka.Align.CENTER):
            view.icon_button("skip_previous").tooltip("Previous").enabled(self.current > 0).on_press(
                self.previous
            )
            view.icon_button("pause" if sound.is_playing else "play_arrow").tooltip(
                "Pause" if sound.is_playing else "Play"
            ).on_press(self.play_pause)
            view.icon_button("skip_next").tooltip("Next").enabled(
                self.current + 1 < len(self.tracks)
            ).on_press(self.next)
            view.text(f"{clock(position)} / {clock(duration)}").text_monospace()
            view.spacer()
            view.checkbox(
                rikka.CheckState.CHECKED if sound.looping else rikka.CheckState.UNCHECKED, "Repeat"
            ).on_toggle(self.repeat)
            view.icon("volume_up")
            view.slider(sound.volume).width(120).on_value(self.set_volume)

    def choose(self, context: rikka.Context) -> None:
        chooser = rikka.FileChooser(rikka.FileChooserPurpose.OPEN, "Choose Music", multiple=True)
        chooser.add_filter("Music")
        chooser.add_filter_mime_type("audio/*")
        context.choose_files(chooser, self.chosen)

    def chosen(self, context: rikka.Context, status: rikka.Status, paths: list[str]) -> None:
        if status == rikka.Status.OK:
            self.tracks = paths
            self.start(context, 0)

    def start(self, context: rikka.Context, index: int) -> None:
        if self.sound is not None:
            self.sound.stop()
        self.current = index
        self.scrub = None
        self.sound = rikka.Sound(self.tracks[index])
        self.sound.on_end(context.window, self.next)
        self.sound.play()

    def pick(self, context: rikka.Context, index: int) -> None:
        self.start(context, index)

    def previous(self, context: rikka.Context) -> None:
        self.start(context, self.current - 1)

    def next(self, context: rikka.Context) -> None:
        if self.current + 1 < len(self.tracks):
            self.start(context, self.current + 1)

    def play_pause(self, context: rikka.Context) -> None:
        if self.sound is None:
            return
        if self.sound.is_playing:
            self.sound.pause()
        else:
            self.sound.play()

    def scrubbing(self, context: rikka.Context, value: float) -> None:
        self.scrub = value

    def seek(self, context: rikka.Context, value: float) -> None:
        self.scrub = None
        if self.sound is not None:
            self.sound.seek(value)

    def repeat(self, context: rikka.Context, on: bool) -> None:
        if self.sound is not None:
            self.sound.looping = on

    def set_volume(self, context: rikka.Context, value: float) -> None:
        if self.sound is not None:
            self.sound.volume = value


def main() -> int:
    player = MusicPlayer()
    app = rikka.App("org.rikka.examples.music_player")
    app.add_window(rikka.WindowBuilder(player.view, title="Music Player", size=(560, 520)))
    try:
        app.run()
    except rikka.Error as error:
        print(f"music_player: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
