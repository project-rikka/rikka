import os.rikka.Align;
import os.rikka.App;
import os.rikka.ButtonVariant;
import os.rikka.Color;
import os.rikka.Context;
import os.rikka.FileChooser;
import os.rikka.FileChooserPurpose;
import os.rikka.Fit;
import os.rikka.PlaybackState;
import os.rikka.Rikka;
import os.rikka.RikkaException;
import os.rikka.Status;
import os.rikka.Video;
import os.rikka.View;
import os.rikka.WindowBuilder;

public final class VideoPlayer {
    private static final Color BLACK = new Color(0, 0, 0, 1);

    private Video video;
    private Double scrub;

    private static String clock(double seconds) {
        int whole = (int) seconds;
        return String.format("%d:%02d", whole / 60, whole % 60);
    }

    private void view(View view) {
        Video shown = video;
        if (shown == null) {
            view.statusPage("movie", "Open a video to play it.", () -> view.button("Open Video…")
                            .onPress(this::choose))
                    .fillWidth()
                    .fillHeight();
            return;
        }
        view.column(() -> {
                    view.videoView(shown).imageFit(Fit.CONTAIN).background(BLACK).fillWidth().grow(1);
                    view.column(() -> {
                                if (!shown.getErrorMessage().isEmpty()) {
                                    view.banner(shown.getErrorMessage());
                                }
                                controls(view, shown);
                            })
                            .gap(Rikka.SPACE_SNUG)
                            .padding(Rikka.SPACE_NORMAL);
                })
                .fillWidth()
                .fillHeight();
    }

    private void controls(View view, Video playing) {
        double duration = playing.getDuration();
        double position = scrub != null ? scrub : playing.getPosition();
        view.slider(position)
                .range(0, Math.max(duration, 1))
                .enabled(duration > 0)
                .onValue((context, value) -> scrub = value)
                .onValueCommit((context, value) -> {
                    scrub = null;
                    playing.seek(value);
                });
        view.row(() -> {
                    view.iconButton(playing.isPlaying() ? "pause" : "play_arrow")
                            .tooltip(playing.isPlaying() ? "Pause" : "Play")
                            .enabled(playing.getState() != PlaybackState.FAILED)
                            .onPress(context -> {
                                if (playing.isPlaying()) {
                                    playing.pause();
                                } else {
                                    playing.play();
                                }
                            });
                    view.iconButton("stop").tooltip("Stop").onPress(context -> playing.stop());
                    view.text(clock(position) + " / " + clock(duration)).textMonospace();
                    view.spacer();
                    view.icon("volume_up");
                    view.slider(playing.getVolume())
                            .width(120)
                            .onValue((context, value) -> playing.setVolume(value));
                    view.button("Open Video…").buttonStyle(ButtonVariant.SECONDARY).onPress(this::choose);
                })
                .gap(Rikka.SPACE_SNUG)
                .alignItems(Align.CENTER);
    }

    private void choose(Context context) {
        FileChooser chooser = new FileChooser(FileChooserPurpose.OPEN, "Open Video")
                .addFilter("Videos")
                .addFilterMimeType("video/*");
        context.chooseFiles(chooser, (chosen, status, paths) -> {
            if (status != Status.OK) {
                return;
            }
            if (video != null) {
                video.stop();
                video.close();
            }
            scrub = null;
            video = new Video(paths.get(0));
            video.play();
        });
    }

    public static void main(String[] arguments) {
        VideoPlayer player = new VideoPlayer();
        WindowBuilder window = new WindowBuilder(player::view).title("Video Player").size(800, 560);
        try {
            new App("org.rikka.examples.video_player").addWindow(window).run();
        } catch (RikkaException exception) {
            System.err.println("video_player: " + exception.getMessage());
            System.exit(1);
        }
    }
}
