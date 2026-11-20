using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using Rikka;

var player = new MusicPlayer();
var window = new WindowBuilder(player.Declare).SetTitle("Music Player").SetSize(560, 520);

try
{
    new App("org.rikka.examples.music_player").AddWindow(window).Run();
}
catch (RikkaException exception)
{
    Console.Error.WriteLine($"music_player: {exception.Message}");
    return 1;
}
return 0;

sealed class MusicPlayer
{
    private IReadOnlyList<string> tracks = [];
    private int current;
    private Sound? sound;
    private double? scrub;

    private static string Clock(double seconds)
    {
        var whole = (int)seconds;
        return string.Create(CultureInfo.InvariantCulture, $"{whole / 60}:{whole % 60:00}");
    }

    public void Declare(View view)
    {
        view.Column(() =>
            {
                view.Row(() =>
                    {
                        view.Text("Music").TextStyle(TextRole.Title).Grow(1);
                        view.Button("Choose Music…").OnPress(Choose);
                    })
                    .Gap(Sdk.SpaceSnug)
                    .AlignItems(Align.Center);
                if (tracks.Count == 0)
                {
                    view.Text("Choose one or more music files to play them in order.").Tone(TextTone.Muted);
                    return;
                }
                view.List(() =>
                    {
                        foreach (var track in tracks)
                        {
                            view.ListItem(Path.GetFileNameWithoutExtension(track)).LeadingIcon("music_note");
                        }
                    })
                    .Selection(current)
                    .OnSelect((context, index) => Start(context, index))
                    .Grow(1);
                if (sound is not null)
                {
                    Controls(view, sound);
                }
            })
            .Gap(Sdk.SpaceRoomy)
            .Padding(Sdk.SpaceLoose)
            .FillWidth()
            .FillHeight();
    }

    private void Controls(View view, Sound playing)
    {
        if (playing.State == PlaybackState.Failed)
        {
            view.Banner(playing.ErrorMessage);
            return;
        }
        var duration = playing.Duration;
        var position = scrub ?? playing.Position;
        view.Slider(position)
            .Range(0, Math.Max(duration, 1))
            .Enabled(duration > 0)
            .OnValue((_, value) => scrub = value)
            .OnValueCommit((_, value) =>
            {
                scrub = null;
                playing.Seek(value);
            });
        view.Row(() =>
            {
                view.IconButton("skip_previous")
                    .Tooltip("Previous")
                    .Enabled(current > 0)
                    .OnPress(context => Start(context, current - 1));
                view.IconButton(playing.IsPlaying ? "pause" : "play_arrow")
                    .Tooltip(playing.IsPlaying ? "Pause" : "Play")
                    .OnPress(_ =>
                    {
                        if (playing.IsPlaying)
                        {
                            playing.Pause();
                        }
                        else
                        {
                            playing.Play();
                        }
                    });
                view.IconButton("skip_next")
                    .Tooltip("Next")
                    .Enabled(current + 1 < tracks.Count)
                    .OnPress(Next);
                view.Text($"{Clock(position)} / {Clock(duration)}").TextMonospace();
                view.Spacer();
                view.Checkbox(playing.Looping ? CheckState.Checked : CheckState.Unchecked, "Repeat")
                    .OnToggle((_, on) => playing.Looping = on);
                view.Icon("volume_up");
                view.Slider(playing.Volume).Width(120).OnValue((_, value) => playing.Volume = value);
            })
            .Gap(Sdk.SpaceSnug)
            .AlignItems(Align.Center);
    }

    private void Choose(Context context)
    {
        var chooser = new FileChooser(FileChooserPurpose.Open, "Choose Music")
            .SetMultiple(true)
            .AddFilter("Music")
            .AddFilterMimeType("audio/*");
        context.ChooseFiles(chooser, (chosen, status, paths) =>
        {
            if (status == Status.OK)
            {
                tracks = paths;
                Start(chosen, 0);
            }
        });
    }

    private void Start(Context context, int index)
    {
        sound?.Stop();
        sound?.Dispose();
        current = index;
        scrub = null;
        sound = new Sound(tracks[index]);
        sound.OnEnd(context.Window, Next);
        sound.Play();
    }

    private void Next(Context context)
    {
        if (current + 1 < tracks.Count)
        {
            Start(context, current + 1);
        }
    }
}
