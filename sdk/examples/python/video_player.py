import sys

import rikka

BLACK = rikka.Color(0, 0, 0, 1)


def clock(seconds: float) -> str:
    whole = int(seconds)
    return f"{whole // 60}:{whole % 60:02d}"


class VideoPlayer:
    def __init__(self) -> None:
        self.video: rikka.Video | None = None
        self.scrub: float | None = None

    def view(self, view: rikka.View) -> None:
        video = self.video
        if video is None:
            with view.status_page("movie", "Open a video to play it.").fill_width().fill_height():
                view.button("Open Video…").on_press(self.choose)
            return
        with view.column().fill_width().fill_height():
            view.video_view(video).image_fit(rikka.Fit.CONTAIN).background(BLACK).fill_width().grow(
                1
            )
            with view.column().gap(rikka.SPACE_SNUG).padding(rikka.SPACE_NORMAL):
                if video.error_message:
                    view.banner(video.error_message)
                self.controls(view, video)

    def controls(self, view: rikka.View, video: rikka.Video) -> None:
        duration = video.duration
        position = video.position if self.scrub is None else self.scrub
        view.slider(position).range(0, max(duration, 1)).enabled(duration > 0).on_value(
            self.scrubbing
        ).on_value_commit(self.seek)
        with view.row().gap(rikka.SPACE_SNUG).align_items(rikka.Align.CENTER):
            view.icon_button("pause" if video.is_playing else "play_arrow").tooltip(
                "Pause" if video.is_playing else "Play"
            ).enabled(video.state != rikka.PlaybackState.FAILED).on_press(self.play_pause)
            view.icon_button("stop").tooltip("Stop").on_press(self.stop)
            view.text(f"{clock(position)} / {clock(duration)}").text_monospace()
            view.spacer()
            view.icon("volume_up")
            view.slider(video.volume).width(120).on_value(self.set_volume)
            view.button("Open Video…").button_style(rikka.ButtonVariant.SECONDARY).on_press(
                self.choose
            )

    def choose(self, context: rikka.Context) -> None:
        chooser = rikka.FileChooser(rikka.FileChooserPurpose.OPEN, "Open Video")
        chooser.add_filter("Videos")
        chooser.add_filter_mime_type("video/*")
        context.choose_files(chooser, self.chosen)

    def chosen(self, context: rikka.Context, status: rikka.Status, paths: list[str]) -> None:
        if status != rikka.Status.OK:
            return
        if self.video is not None:
            self.video.stop()
        self.scrub = None
        self.video = rikka.Video(paths[0])
        self.video.play()

    def play_pause(self, context: rikka.Context) -> None:
        if self.video is None:
            return
        if self.video.is_playing:
            self.video.pause()
        else:
            self.video.play()

    def stop(self, context: rikka.Context) -> None:
        if self.video is not None:
            self.video.stop()

    def scrubbing(self, context: rikka.Context, value: float) -> None:
        self.scrub = value

    def seek(self, context: rikka.Context, value: float) -> None:
        self.scrub = None
        if self.video is not None:
            self.video.seek(value)

    def set_volume(self, context: rikka.Context, value: float) -> None:
        if self.video is not None:
            self.video.volume = value


def main() -> int:
    player = VideoPlayer()
    app = rikka.App("org.rikka.examples.video_player")
    app.add_window(rikka.WindowBuilder(player.view, title="Video Player", size=(800, 560)))
    try:
        app.run()
    except rikka.Error as error:
        print(f"video_player: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
