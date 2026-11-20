using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using Rikka;

internal sealed class Notes
{
    public TextDocument Document { get; } = new("draft");
    public Image Picture { get; set; } = Image.FromFile("/nonexistent.png");
    public Video Clip { get; } = new("/nonexistent.webm");
    public List<string> Items { get; } = ["one", "two"];
    public int Page { get; set; }
    public bool Dark { get; set; }
}

internal static class Program
{
    private static int failures;

    private static void Check(
        bool condition,
        [CallerArgumentExpression(nameof(condition))] string? expression = null,
        [CallerLineNumber] int line = 0)
    {
        if (!condition)
        {
            Console.Error.WriteLine($"Api.cs:{line}: check failed: {expression}");
            failures++;
        }
    }

    private static void Throws<TException>(Action action, [CallerLineNumber] int line = 0)
        where TException : Exception
    {
        try
        {
            action();
        }
        catch (TException)
        {
            return;
        }
        Console.Error.WriteLine($"Api.cs:{line}: expected {typeof(TException).Name}");
        failures++;
    }

    private static void Declare(View view, Notes notes)
    {
        notes.Dark = view.IsDark;
        Color accent = view.PaletteColor(ColorRole.Accent);
        view.Column(() =>
            {
                view.Text("Notes").TextStyle(TextRole.Title).Tint(accent);
                view.Row(() =>
                    {
                        view.Button("Save").ButtonStyle(ButtonVariant.Primary).OnPress(context =>
                        {
                            context.SetClipboardText(notes.Document.Text);
                            context.After("autosave", 500, later => later.SkipView());
                        });
                        view.Toggle(notes.Dark).OnToggle((context, on) => notes.Dark = on);
                        view.Slider(0.5).Range(0, 2).OnValue((context, value) => { });
                    })
                    .Gap(Sdk.SpaceSnug);
                view.Segmented(notes.Page, () =>
                    {
                        view.Segment("Edit");
                        view.Segment("Preview");
                    })
                    .OnSelect((context, page) => notes.Page = page);
                view.TextEditor(notes.Document).Code(true).Syntax("rust").OnChange(context => { });
                view.ImageView(notes.Picture)
                    .ImageFit(Fit.Contain)
                    .OnDropFiles((context, paths) => notes.Picture = Image.FromFile(paths[0]))
                    .OnDropText((context, text) => notes.Document.Insert(text));
                view.VideoView(notes.Clip).ImageFit(Fit.Cover).Height(120);
                view.IconButton("play_arrow").Tooltip("Play").OnPress(context =>
                {
                    notes.Clip.OnEnd(context.Window, ended =>
                        ended.Confirm("Again?", "The clip ended.", "Replay", (answered, replay) => { }));
                    notes.Clip.Play();
                });
                view.LazyList(notes.Items.Count, (row, index, selected) =>
                        row.ListItem(notes.Items[index]).Check(selected ? CheckState.Checked : CheckState.Unchecked))
                    .ItemExtent(32)
                    .OnActivate((context, index) => context.ReadClipboardText((done, present, text) => { }));
                view.Memo(notes.Document.Revision, content => content.Spinner());
                view.UnsavedPrompt("draft", false, (context, answer) =>
                {
                    if (answer == UnsavedAnswer.Discard)
                    {
                        context.Request(WindowRequest.Close);
                    }
                });
            })
            .Padding(Sdk.SpaceLoose)
            .OnShortcut((KeyCode)'s', Modifiers.Control, context =>
            {
                var chooser = new FileChooser(FileChooserPurpose.Save, "Save").SetName("notes.txt");
                context.ChooseFiles(chooser, (done, status, paths) =>
                {
                    if (status == Status.OK)
                    {
                        done.Window.SetTitle(paths[0]);
                    }
                });
                context.RunInBackground(() => { }, done => done.Window.Invalidate());
            });
    }

    private static void VersionMatchesBinding()
    {
        Check(Sdk.Version() == Sdk.BindingVersion);
        Check(Sdk.LastError() == string.Empty);
        Check(Sdk.None == -1);
    }

    private static void TextRoundTripsUtf8()
    {
        using var document = new TextDocument("日本語 text");
        Check(document.Text == "日本語 text");
        Check(document.Length == System.Text.Encoding.UTF8.GetByteCount("日本語 text"));
        document.Text = "terminated ✓\0inside";
        Check(document.Text == "terminated ✓\0inside");
        document.Text = string.Empty;
        Check(document.Length == 0);
        Check(document.Text.Length == 0);
    }

    private static void DocumentsEditUndoAndRedo()
    {
        using var document = new TextDocument("first");
        Check(!document.IsModified);
        Check(!document.CanUndo);
        ulong before = document.Revision;
        document.Text = "second";
        Check(document.Revision != before);
        Check(document.IsModified);
        Check(document.Undo());
        Check(document.Text == "first");
        Check(document.Redo());
        document.MarkSaved();
        Check(!document.IsModified);
        document.SelectAll();
        document.Insert("new");
        Check(document.Text == "new");
        Check(document.SelectedText.Length == 0);
        document.Reset("fresh");
        Check(!document.CanUndo);
        Check(!document.Undo());
    }

    private static void DisposedObjectsRefuseUse()
    {
        var document = new TextDocument("gone");
        document.Dispose();
        Throws<ObjectDisposedException>(() => _ = document.Text);
        Throws<ArgumentNullException>(() => _ = new TextDocument(null!));
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static WeakReference DropDocuments()
    {
        var kept = new object();
        for (int index = 0; index < 8; index++)
        {
            _ = new TextDocument($"draft {index}");
        }
        return new WeakReference(kept);
    }

    private static void DocumentsFinalizedElsewhereAreReleasedOnTheirThread()
    {
        DropDocuments();
        GC.Collect();
        GC.WaitForPendingFinalizers();
        using var next = new TextDocument("next");
        Check(next.Text == "next");
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static WeakReference Builder(bool dispose)
    {
        var tracker = new object();
        var builder = new WindowBuilder(view => GC.KeepAlive(tracker))
            .SetTitle("Alone")
            .SetSize(320, 200)
            .OnStart(context => GC.KeepAlive(tracker))
            .AddShortcut((KeyCode)'s', Modifiers.Control, context => GC.KeepAlive(tracker));
        if (dispose)
        {
            builder.Dispose();
        }
        return new WeakReference(tracker);
    }

    private static bool Collected(WeakReference reference)
    {
        for (int attempt = 0; attempt < 10 && reference.IsAlive; attempt++)
        {
            GC.Collect();
            GC.WaitForPendingFinalizers();
        }
        return !reference.IsAlive;
    }

    private static void BuildersReleaseTheirClosures()
    {
        Check(Collected(Builder(dispose: true)));
        Check(Collected(Builder(dispose: false)));
    }

    private static void HandingOverConsumesTheBuilder()
    {
        var window = new WindowBuilder(view => { });
        using var app = new App("org.rikka.sdk.test").AddWindow(window).SetSingleInstance(false);
        Throws<ObjectDisposedException>(() => window.SetTitle("Again"));
        Throws<ArgumentNullException>(() => app.AddWindow(null!));
    }

    private static void ImagesShareOneReference()
    {
        var pixels = new byte[2 * 3 * 4];
        Array.Fill(pixels, (byte)0x7f);
        using var rgba = Image.FromRgba(2, 3, pixels);
        Check(rgba.Equals(rgba));
        Check(!rgba.Equals(Image.FromEncoded([0x89, (byte)'P', (byte)'N', (byte)'G'])));
    }

    private static void MissingSoundsFailAndKeepSettings()
    {
        using var sound = new Sound("/nonexistent.ogg");
        Check(sound.State == PlaybackState.Failed);
        Check(sound.ErrorMessage.Contains("/nonexistent.ogg", StringComparison.Ordinal));
        sound.Volume = 0.5;
        sound.Looping = true;
        Check(sound.Volume == 0.5);
        Check(sound.Looping);
    }

    private static void OpeningARelativeUrlThrows()
    {
        try
        {
            Sdk.OpenUrl("example.org");
            Check(false);
        }
        catch (RikkaException exception)
        {
            Check(exception.Status == Status.Failed);
        }
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static (Status? Status, string Message, WeakReference Tracker) RunWithoutDisplay()
    {
        var tracker = new object();
        var notes = new Notes();
        var app = new App("org.rikka.sdk.test").AddWindow(new WindowBuilder(view =>
            {
                GC.KeepAlive(tracker);
                Declare(view, notes);
            })
            .SetTitle("Never shown"));
        try
        {
            app.Run();
            return (null, string.Empty, new WeakReference(tracker));
        }
        catch (RikkaException exception)
        {
            Throws<ObjectDisposedException>(app.Run);
            return (exception.Status, exception.Message, new WeakReference(tracker));
        }
    }

    private static void RunningWithoutADisplayThrows()
    {
        var (status, message, tracker) = RunWithoutDisplay();
        Check(status == Status.NoDisplay);
        Check(message.Length > 0);
        Check(Collected(tracker));
    }

    private static int Main()
    {
        VersionMatchesBinding();
        TextRoundTripsUtf8();
        DocumentsEditUndoAndRedo();
        DisposedObjectsRefuseUse();
        DocumentsFinalizedElsewhereAreReleasedOnTheirThread();
        BuildersReleaseTheirClosures();
        HandingOverConsumesTheBuilder();
        ImagesShareOneReference();
        MissingSoundsFailAndKeepSettings();
        OpeningARelativeUrlThrows();
        RunningWithoutADisplayThrows();
        if (failures > 0)
        {
            Console.Error.WriteLine($"{failures} checks failed");
            return 1;
        }
        return 0;
    }
}
