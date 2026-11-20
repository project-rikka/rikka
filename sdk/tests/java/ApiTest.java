import java.lang.ref.WeakReference;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import java.util.function.BooleanSupplier;
import os.rikka.App;
import os.rikka.ButtonVariant;
import os.rikka.CheckState;
import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.Context;
import os.rikka.FileChooser;
import os.rikka.FileChooserPurpose;
import os.rikka.Fit;
import os.rikka.Image;
import os.rikka.KeyCode;
import os.rikka.Modifier;
import os.rikka.PlaybackState;
import os.rikka.Rikka;
import os.rikka.RikkaException;
import os.rikka.Sound;
import os.rikka.Status;
import os.rikka.TextDocument;
import os.rikka.TextRole;
import os.rikka.UnsavedAnswer;
import os.rikka.Video;
import os.rikka.View;
import os.rikka.WindowBuilder;
import os.rikka.WindowRequest;

public final class ApiTest {
    private static int failures;

    private static final class Notes {
        final TextDocument document = new TextDocument("draft");
        Image picture = Image.fromFile("/nonexistent.png");
        final Video clip = new Video("/nonexistent.webm");
        final List<String> items = List.of("one", "two");
        int page;
        boolean dark;
    }

    private static void check(boolean condition, String what) {
        if (!condition) {
            System.err.println("check failed: " + what);
            failures++;
        }
    }

    private static void throwing(Class<? extends Throwable> expected, Runnable action, String what) {
        try {
            action.run();
        } catch (Throwable failure) {
            if (expected.isInstance(failure)) {
                return;
            }
            System.err.println(what + ": threw " + failure);
            failures++;
            return;
        }
        System.err.println(what + ": expected " + expected.getSimpleName());
        failures++;
    }

    static void declare(View view, Notes notes) {
        notes.dark = view.isDark();
        Color accent = view.paletteColor(ColorRole.ACCENT);
        view.column(() -> {
                    view.text("Notes").textStyle(TextRole.TITLE).tint(accent);
                    view.row(() -> {
                                view.button("Save").buttonStyle(ButtonVariant.PRIMARY).onPress(context -> {
                                    context.setClipboardText(notes.document.getText());
                                    context.after("autosave", 500, Context::skipView);
                                });
                                view.toggle(notes.dark).onToggle((context, on) -> notes.dark = on);
                                view.slider(0.5).range(0, 2).onValue((context, value) -> {});
                            })
                            .gap(Rikka.SPACE_SNUG);
                    view.segmented(notes.page, () -> {
                                view.segment("Edit");
                                view.segment("Preview");
                            })
                            .onSelect((context, page) -> notes.page = page);
                    view.textEditor(notes.document).code(true).syntax("rust").onChange(context -> {});
                    view.imageView(notes.picture)
                            .imageFit(Fit.CONTAIN)
                            .onDropFiles((context, paths) -> notes.picture = Image.fromFile(paths.get(0)))
                            .onDropText((context, text) -> notes.document.insert(text));
                    view.videoView(notes.clip).imageFit(Fit.COVER).height(120);
                    view.iconButton("play_arrow").tooltip("Play").onPress(context -> {
                        notes.clip.onEnd(context.getWindow(), ended ->
                                ended.confirm("Again?", "The clip ended.", "Replay", (answered, replay) -> {}));
                        notes.clip.play();
                    });
                    view.lazyList(notes.items.size(), (row, index, selected) ->
                                    row.listItem(notes.items.get(index))
                                            .check(selected ? CheckState.CHECKED : CheckState.UNCHECKED))
                            .itemExtent(32)
                            .onActivate((context, index) ->
                                    context.readClipboardText((done, present, text) -> {}));
                    view.memo(notes.document.getRevision(), View::spinner);
                    view.unsavedPrompt("draft", false, (context, answer) -> {
                        if (answer == UnsavedAnswer.DISCARD) {
                            context.request(WindowRequest.CLOSE);
                        }
                    });
                })
                .padding(Rikka.SPACE_LOOSE)
                .onShortcut(KeyCode.of('s'), EnumSet.of(Modifier.CONTROL), context -> {
                    FileChooser chooser = new FileChooser(FileChooserPurpose.SAVE, "Save").name("notes.txt");
                    context.chooseFiles(chooser, (done, status, paths) -> {
                        if (status == Status.OK) {
                            done.getWindow().setTitle(paths.get(0));
                        }
                    });
                    context.runInBackground(() -> {}, done -> done.getWindow().invalidate());
                });
    }

    private static boolean collected(BooleanSupplier alive) {
        for (int attempt = 0; attempt < 50 && alive.getAsBoolean(); attempt++) {
            System.gc();
            try {
                Thread.sleep(10);
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                return false;
            }
        }
        return !alive.getAsBoolean();
    }

    private static void versionMatchesBinding() {
        check(Rikka.version() == Rikka.BINDING_VERSION, "version");
        check(Rikka.lastError().isEmpty(), "no error yet");
        check(Rikka.NONE == -1, "none");
        check(KeyCode.of('s').value() == 's', "open key code");
    }

    private static void textRoundTripsUtf8() {
        try (TextDocument document = new TextDocument("日本語 text")) {
            check(document.getText().equals("日本語 text"), "UTF-8 text");
            check(document.getLength() == "日本語 text".getBytes(StandardCharsets.UTF_8).length, "length in bytes");
            document.setText("terminated ✓\0inside");
            check(document.getText().equals("terminated ✓\0inside"), "embedded NUL");
            document.setText("");
            check(document.getLength() == 0 && document.getText().isEmpty(), "empty text");
        }
    }

    private static void documentsEditUndoAndRedo() {
        try (TextDocument document = new TextDocument("first")) {
            check(!document.isModified() && !document.canUndo(), "a new document is pristine");
            long before = document.getRevision();
            document.setText("second");
            check(document.getRevision() != before && document.isModified(), "an edit registers");
            check(document.undo() && document.getText().equals("first"), "undo");
            check(document.redo(), "redo");
            document.markSaved();
            check(!document.isModified(), "saved");
            document.selectAll();
            document.insert("new");
            check(document.getText().equals("new") && document.getSelectedText().isEmpty(), "insert");
            document.reset("fresh");
            check(!document.canUndo() && !document.undo(), "reset forgets history");
        }
    }

    private static void closedObjectsRefuseUse() {
        TextDocument document = new TextDocument("gone");
        document.close();
        throwing(IllegalStateException.class, document::getText, "closed document");
        throwing(NullPointerException.class, () -> new TextDocument(null), "null text");
    }

    private static void dropDocuments() {
        for (int index = 0; index < 8; index++) {
            new TextDocument("draft " + index).setText("dropped");
        }
    }

    private static void documentsCleanedElsewhereAreReleasedOnTheirThread() {
        dropDocuments();
        WeakReference<Object> probe = new WeakReference<>(new Object());
        check(collected(() -> probe.get() != null), "a collection ran");
        try (TextDocument next = new TextDocument("next")) {
            check(next.getText().equals("next"), "deferred releases drained");
        }
    }

    private static WeakReference<Object> builder(boolean close) {
        Object tracker = new Object();
        WindowBuilder builder = new WindowBuilder(view -> tracker.hashCode())
                .title("Alone")
                .size(320, 200)
                .onStart(context -> tracker.hashCode())
                .addShortcut(KeyCode.of('s'), EnumSet.of(Modifier.CONTROL), context -> tracker.hashCode());
        if (close) {
            builder.close();
        }
        return new WeakReference<>(tracker);
    }

    private static void buildersReleaseTheirClosures() {
        WeakReference<Object> closed = builder(true);
        check(collected(() -> closed.get() != null), "a closed builder's closures");
        WeakReference<Object> dropped = builder(false);
        check(collected(() -> dropped.get() != null), "a dropped builder's closures");
    }

    private static void handingOverConsumesTheBuilder() {
        WindowBuilder window = new WindowBuilder(view -> {});
        try (App app = new App("org.rikka.sdk.test").addWindow(window).singleInstance(false)) {
            throwing(IllegalStateException.class, () -> window.title("Again"), "handed-over builder");
            throwing(NullPointerException.class, () -> app.addWindow(null), "null builder");
        }
    }

    private static void imagesShareOneReference() {
        byte[] pixels = new byte[2 * 3 * 4];
        Arrays.fill(pixels, (byte) 0x7f);
        try (Image rgba = Image.fromRgba(2, 3, pixels);
                Image encoded = Image.fromEncoded(new byte[] {(byte) 0x89, 'P', 'N', 'G'})) {
            check(rgba.equals(rgba) && rgba.hashCode() == rgba.hashCode(), "an image equals itself");
            check(!rgba.equals(encoded), "different images differ");
        }
    }

    private static void missingSoundsFailAndKeepSettings() {
        try (Sound sound = new Sound("/nonexistent.ogg")) {
            check(sound.getState() == PlaybackState.FAILED, "a missing sound fails");
            check(sound.getErrorMessage().contains("/nonexistent.ogg"), "the failure names the file");
            sound.setVolume(0.5);
            sound.setLooping(true);
            check(sound.getVolume() == 0.5 && sound.isLooping(), "settings are kept");
        }
    }

    private static void openingARelativeUrlThrows() {
        try {
            Rikka.openUrl("example.org");
            check(false, "a relative URL is refused");
        } catch (RikkaException failure) {
            check(failure.status() == Status.FAILED, "a refused URL fails");
        }
    }

    private static WeakReference<Object> runWithoutDisplay(Status[] status, String[] message) {
        Object tracker = new Object();
        Notes notes = new Notes();
        App app = new App("org.rikka.sdk.test")
                .addWindow(new WindowBuilder(view -> {
                            tracker.hashCode();
                            declare(view, notes);
                        })
                        .title("Never shown"));
        try {
            app.run();
        } catch (RikkaException failure) {
            status[0] = failure.status();
            message[0] = failure.getMessage();
        }
        throwing(IllegalStateException.class, app::run, "a consumed app");
        return new WeakReference<>(tracker);
    }

    private static void runningWithoutADisplayThrows() {
        Status[] status = new Status[1];
        String[] message = new String[1];
        WeakReference<Object> tracker = runWithoutDisplay(status, message);
        check(status[0] == Status.NO_DISPLAY, "no display");
        check(message[0] != null && !message[0].isEmpty(), "a message");
        check(collected(() -> tracker.get() != null), "the window's closure");
    }

    public static void main(String[] arguments) {
        versionMatchesBinding();
        textRoundTripsUtf8();
        documentsEditUndoAndRedo();
        closedObjectsRefuseUse();
        documentsCleanedElsewhereAreReleasedOnTheirThread();
        buildersReleaseTheirClosures();
        handingOverConsumesTheBuilder();
        imagesShareOneReference();
        missingSoundsFailAndKeepSettings();
        openingARelativeUrlThrows();
        runningWithoutADisplayThrows();
        if (failures > 0) {
            System.err.println(failures + " checks failed");
            System.exit(1);
        }
    }
}
