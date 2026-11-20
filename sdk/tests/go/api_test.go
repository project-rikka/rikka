package rikka_test

import (
	"errors"
	"runtime"
	"strings"
	"testing"
	"time"

	rikka "github.com/project-rikka/rikka/sdk/bindings/go"
)

type notes struct {
	document *rikka.TextDocument
	picture  *rikka.Image
	clip     *rikka.Video
	items    []string
	page     int
	dark     bool
}

func declare(view *rikka.View, notes *notes) {
	notes.dark = view.IsDark()
	accent := view.PaletteColor(rikka.ColorRoleAccent)
	view.Column(func() {
		view.Text("Notes").TextStyle(rikka.TextRoleTitle).Tint(accent)
		view.Row(func() {
			view.Button("Save").ButtonStyle(rikka.ButtonVariantPrimary).OnPress(func(context *rikka.Context) {
				context.SetClipboardText(notes.document.Text())
				context.After("autosave", 500, func(later *rikka.Context) { later.SkipView() })
			})
			view.Toggle(notes.dark).OnToggle(func(context *rikka.Context, on bool) { notes.dark = on })
			view.Slider(0.5).Range(0, 2).OnValue(func(*rikka.Context, float64) {})
		}).Gap(rikka.SpaceSnug)
		view.Segmented(notes.page, func() {
			view.Segment("Edit")
			view.Segment("Preview")
		}).OnSelect(func(context *rikka.Context, page int) { notes.page = page })
		view.TextEditor(notes.document).Code(true).Syntax("rust").OnChange(func(*rikka.Context) {})
		view.ImageView(notes.picture).
			ImageFit(rikka.FitContain).
			OnDropFiles(func(_ *rikka.Context, paths []string) { notes.picture = rikka.NewImageFromFile(paths[0]) }).
			OnDropText(func(_ *rikka.Context, text string) { notes.document.Insert(text) })
		view.VideoView(notes.clip).ImageFit(rikka.FitCover).Height(120)
		view.IconButton("play_arrow").Tooltip("Play").OnPress(func(context *rikka.Context) {
			notes.clip.OnEnd(context.Window(), func(ended *rikka.Context) {
				ended.Confirm("Again?", "The clip ended.", "Replay", func(*rikka.Context, bool) {})
			})
			notes.clip.Play()
		})
		view.LazyList(len(notes.items), func(row *rikka.View, index int, selected bool) {
			row.ListItem(notes.items[index]).Check(rikka.CheckStateChecked)
		}).ItemExtent(32).OnActivate(func(context *rikka.Context, index int) {
			context.ReadClipboardText(func(context *rikka.Context, present bool, text string) {})
		})
		view.Memo(notes.document.Revision(), func(content *rikka.View) { content.Spinner() })
		view.UnsavedPrompt("draft", false, func(context *rikka.Context, answer rikka.UnsavedAnswer) {
			if answer == rikka.UnsavedAnswerDiscard {
				context.Request(rikka.WindowRequestClose)
			}
		})
	}).Padding(rikka.SpaceLoose).OnShortcut('s', rikka.ModifierControl, func(context *rikka.Context) {
		chooser := rikka.NewFileChooser(rikka.FileChooserPurposeSave, "Save").Name("notes.txt")
		context.ChooseFiles(chooser, func(done *rikka.Context, err error, paths []string) {
			if err == nil {
				done.Window().SetTitle(paths[0])
			}
		})
		context.RunInBackground(func() {}, func(done *rikka.Context) { done.Window().Invalidate() })
	})
}

func collected(t *testing.T, done <-chan struct{}) bool {
	t.Helper()
	for range 100 {
		runtime.GC()
		select {
		case <-done:
			return true
		case <-time.After(10 * time.Millisecond):
		}
	}
	return false
}

func tracked(done chan struct{}) *int {
	tracker := new(int)
	runtime.AddCleanup(tracker, func(done chan struct{}) { close(done) }, done)
	return tracker
}

func panics(f func()) (message any) {
	defer func() { message = recover() }()
	f()
	return nil
}

func TestVersionMatchesBinding(t *testing.T) {
	if got := rikka.Version(); got != rikka.BindingVersion {
		t.Fatalf("Version() = %#x, want %#x", got, rikka.BindingVersion)
	}
	if got := rikka.LastError(); got != "" {
		t.Fatalf("LastError() = %q, want empty", got)
	}
}

func TestValuesNameThemselves(t *testing.T) {
	cases := []struct {
		got, want string
	}{
		{rikka.StatusNoDisplay.String(), "StatusNoDisplay"},
		{(rikka.ModifierControl | rikka.ModifierShift).String(), "ModifierShift|ModifierControl"},
		{rikka.KeyCode('s').String(), "KeyCode(0x73)"},
		{rikka.KeyEnter.String(), "KeyEnter"},
		{rikka.StatusClosed.Error(), "rikka: closed"},
	}
	for _, c := range cases {
		if c.got != c.want {
			t.Errorf("got %q, want %q", c.got, c.want)
		}
	}
	if rikka.None != -1 {
		t.Errorf("None = %d, want -1", rikka.None)
	}
}

func TestTextRoundTripsUTF8(t *testing.T) {
	runtime.LockOSThread()
	document := rikka.NewTextDocument("日本語 text")
	if got := document.Text(); got != "日本語 text" {
		t.Fatalf("Text() = %q", got)
	}
	if got := document.Length(); got != len("日本語 text") {
		t.Fatalf("Length() = %d", got)
	}
	document.SetText("terminated ✓\x00inside")
	if got := document.Text(); got != "terminated ✓\x00inside" {
		t.Fatalf("Text() = %q", got)
	}
	document.SetText("")
	if document.Length() != 0 || document.Text() != "" {
		t.Fatalf("empty text round trip failed")
	}
}

func TestDocumentEditsUndoAndRedo(t *testing.T) {
	runtime.LockOSThread()
	document := rikka.NewTextDocument("first")
	if document.IsModified() || document.CanUndo() {
		t.Fatal("a new document is modified or undoable")
	}
	before := document.Revision()
	document.SetText("second")
	if document.Revision() == before || !document.IsModified() {
		t.Fatal("an edit did not register")
	}
	if !document.Undo() || document.Text() != "first" {
		t.Fatal("undo failed")
	}
	if !document.Redo() {
		t.Fatal("redo failed")
	}
	document.MarkSaved()
	if document.IsModified() {
		t.Fatal("a saved document is modified")
	}
	document.SelectAll()
	document.Insert("new")
	if document.Text() != "new" || document.SelectedText() != "" {
		t.Fatalf("Text() = %q after replacing the selection", document.Text())
	}
	document.Reset("fresh")
	if document.CanUndo() || document.Undo() {
		t.Fatal("a reset document can undo")
	}
}

func TestDocumentsCollectedOnOtherThreadsAreReleasedOnTheirOwn(t *testing.T) {
	runtime.LockOSThread()
	done := make(chan struct{})
	func() {
		for range 8 {
			rikka.NewTextDocument("draft").SetText("dropped")
		}
		runtime.AddCleanup(new(int), func(done chan struct{}) { close(done) }, done)
	}()
	if !collected(t, done) {
		t.Fatal("nothing was collected")
	}
	if got := rikka.NewTextDocument("next").Text(); got != "next" {
		t.Fatalf("Text() = %q", got)
	}
}

func TestBuildersReleaseTheirClosures(t *testing.T) {
	runtime.LockOSThread()
	done := make(chan struct{})
	func() {
		tracker := tracked(done)
		rikka.NewWindowBuilder(func(*rikka.View) { _ = tracker }).
			Title("Alone").
			Size(320, 200).
			OnStart(func(*rikka.Context) { _ = tracker }).
			AddShortcut('s', rikka.ModifierControl, func(*rikka.Context) { _ = tracker })
	}()
	if !collected(t, done) {
		t.Fatal("the closures of a dropped builder were kept")
	}
}

func TestHandingOverConsumesTheBuilder(t *testing.T) {
	runtime.LockOSThread()
	window := rikka.NewWindowBuilder(func(*rikka.View) {})
	rikka.NewApp("org.rikka.sdk.test").AddWindow(window)
	if message := panics(func() { window.Title("Again") }); message != "rikka: the WindowBuilder was consumed" {
		t.Fatalf("reusing a handed-over builder: %v", message)
	}
	if message := panics(func() { rikka.NewWindowBuilder(nil) }); message != "rikka: view is nil" {
		t.Fatalf("a nil callback: %v", message)
	}
}

func TestImagesAreMadeThreeWays(t *testing.T) {
	pixels := make([]byte, 2*3*4)
	for index := range pixels {
		pixels[index] = 0x7f
	}
	images := []*rikka.Image{
		rikka.NewImageFromRGBA(2, 3, pixels),
		rikka.NewImageFromEncoded([]byte{0x89, 'P', 'N', 'G'}),
		rikka.NewImageFromFile("/nonexistent.png"),
	}
	for _, image := range images {
		if image == nil {
			t.Fatal("an image was not made")
		}
	}
}

func TestMissingSoundsFailAndKeepSettings(t *testing.T) {
	sound := rikka.NewSound("/nonexistent.ogg")
	if state := sound.State(); state != rikka.PlaybackStateFailed {
		t.Fatalf("State() = %v, want PlaybackStateFailed", state)
	}
	if message := sound.ErrorMessage(); !strings.Contains(message, "/nonexistent.ogg") {
		t.Fatalf("ErrorMessage() = %q, want it to name the file", message)
	}
	sound.SetVolume(0.5)
	sound.SetLooping(true)
	if sound.Volume() != 0.5 || !sound.Looping() {
		t.Fatalf("Volume() = %v, Looping() = %v after setting them", sound.Volume(), sound.Looping())
	}
}

func TestOpeningARelativeURLFails(t *testing.T) {
	err := rikka.OpenURL("example.org")
	if !errors.Is(err, rikka.StatusFailed) {
		t.Fatalf("OpenURL() = %v, want StatusFailed", err)
	}
}

func TestRunningWithoutADisplayFails(t *testing.T) {
	runtime.LockOSThread()
	done := make(chan struct{})
	var err error
	func() {
		tracker := tracked(done)
		notes := &notes{
			document: rikka.NewTextDocument("draft"),
			picture:  rikka.NewImageFromFile("/nonexistent.png"),
			clip:     rikka.NewVideo("/nonexistent.webm"),
			items:    []string{"one", "two"},
		}
		window := rikka.NewWindowBuilder(func(view *rikka.View) {
			_ = tracker
			declare(view, notes)
		}).Title("Never shown")
		err = rikka.NewApp("org.rikka.sdk.test").AddWindow(window).Run()
	}()
	if !errors.Is(err, rikka.StatusNoDisplay) {
		t.Fatalf("Run() = %v, want StatusNoDisplay", err)
	}
	var failure *rikka.Error
	if !errors.As(err, &failure) || failure.Message == "" {
		t.Fatalf("Run() = %#v, want an *Error with a message", err)
	}
	if !collected(t, done) {
		t.Fatal("the window's closure was kept")
	}
}
