import java.nio.file.Path;
import java.util.List;
import os.rikka.Align;
import os.rikka.App;
import os.rikka.CheckState;
import os.rikka.Context;
import os.rikka.FileChooser;
import os.rikka.FileChooserPurpose;
import os.rikka.PlaybackState;
import os.rikka.Rikka;
import os.rikka.RikkaException;
import os.rikka.Sound;
import os.rikka.Status;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.View;
import os.rikka.WindowBuilder;

public final class MusicPlayer {
    private List<String> tracks = List.of();
    private int current;
    private Sound sound;
    private Double scrub;

    private static String clock(double seconds) {
        int whole = (int) seconds;
        return String.format("%d:%02d", whole / 60, whole % 60);
    }

    private static String stem(String path) {
        String name = Path.of(path).getFileName().toString();
        int dot = name.lastIndexOf('.');
        return dot > 0 ? name.substring(0, dot) : name;
    }

    private void view(View view) {
        view.column(() -> {
                    view.row(() -> {
                                view.text("Music").textStyle(TextRole.TITLE).grow(1);
                                view.button("Choose Music…").onPress(this::choose);
                            })
                            .gap(Rikka.SPACE_SNUG)
                            .alignItems(Align.CENTER);
                    if (tracks.isEmpty()) {
                        view.text("Choose one or more music files to play them in order.")
                                .tone(TextTone.MUTED);
                        return;
                    }
                    view.list(() -> {
                                for (String track : tracks) {
                                    view.listItem(stem(track)).leadingIcon("music_note");
                                }
                            })
                            .selection(current)
                            .onSelect(this::start)
                            .grow(1);
                    if (sound != null) {
                        controls(view, sound);
                    }
                })
                .gap(Rikka.SPACE_ROOMY)
                .padding(Rikka.SPACE_LOOSE)
                .fillWidth()
                .fillHeight();
    }

    private void controls(View view, Sound playing) {
        if (playing.getState() == PlaybackState.FAILED) {
            view.banner(playing.getErrorMessage());
            return;
        }
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
                    view.iconButton("skip_previous")
                            .tooltip("Previous")
                            .enabled(current > 0)
                            .onPress(context -> start(context, current - 1));
                    view.iconButton(playing.isPlaying() ? "pause" : "play_arrow")
                            .tooltip(playing.isPlaying() ? "Pause" : "Play")
                            .onPress(context -> {
                                if (playing.isPlaying()) {
                                    playing.pause();
                                } else {
                                    playing.play();
                                }
                            });
                    view.iconButton("skip_next")
                            .tooltip("Next")
                            .enabled(current + 1 < tracks.size())
                            .onPress(this::next);
                    view.text(clock(position) + " / " + clock(duration)).textMonospace();
                    view.spacer();
                    view.checkbox(playing.isLooping() ? CheckState.CHECKED : CheckState.UNCHECKED, "Repeat")
                            .onToggle((context, on) -> playing.setLooping(on));
                    view.icon("volume_up");
                    view.slider(playing.getVolume())
                            .width(120)
                            .onValue((context, value) -> playing.setVolume(value));
                })
                .gap(Rikka.SPACE_SNUG)
                .alignItems(Align.CENTER);
    }

    private void choose(Context context) {
        FileChooser chooser = new FileChooser(FileChooserPurpose.OPEN, "Choose Music")
                .multiple(true)
                .addFilter("Music")
                .addFilterMimeType("audio/*");
        context.chooseFiles(chooser, (chosen, status, paths) -> {
            if (status == Status.OK) {
                tracks = paths;
                start(chosen, 0);
            }
        });
    }

    private void start(Context context, int index) {
        if (sound != null) {
            sound.stop();
            sound.close();
        }
        current = index;
        scrub = null;
        sound = new Sound(tracks.get(index));
        sound.onEnd(context.getWindow(), this::next);
        sound.play();
    }

    private void next(Context context) {
        if (current + 1 < tracks.size()) {
            start(context, current + 1);
        }
    }

    public static void main(String[] arguments) {
        MusicPlayer player = new MusicPlayer();
        WindowBuilder window = new WindowBuilder(player::view).title("Music Player").size(560, 520);
        try {
            new App("org.rikka.examples.music_player").addWindow(window).run();
        } catch (RikkaException exception) {
            System.err.println("music_player: " + exception.getMessage());
            System.exit(1);
        }
    }
}
