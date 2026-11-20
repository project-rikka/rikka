using System;
using System.Globalization;
using Rikka;

var player = new VideoPlayer();
var window = new WindowBuilder(player.Declare).SetTitle("Video Player").SetSize(800, 560);

try
{
    new App("org.rikka.examples.video_player").AddWindow(window).Run();
}
catch (RikkaException exception)
{
    Console.Error.WriteLine($"video_player: {exception.Message}");
    return 1;
}
return 0;

sealed class VideoPlayer
{
    private static readonly Color Black = new(0, 0, 0, 1);

    private Video? video;
    private double? scrub;

    private static string Clock(double seconds)
    {
        var whole = (int)seconds;
        return string.Create(CultureInfo.InvariantCulture, $"{whole / 60}:{whole % 60:00}");
    }

    public void Declare(View view)
    {
        if (video is null)
        {
            view.StatusPage("movie", "Open a video to play it.", () => view.Button("Open Video…").OnPress(Choose))
                .FillWidth()
                .FillHeight();
            return;
        }
        var shown = video;
        view.Column(() =>
            {
                view.VideoView(shown).ImageFit(Fit.Contain).Background(Black).FillWidth().Grow(1);
                view.Column(() =>
                    {
                        if (shown.ErrorMessage.Length > 0)
                        {
                            view.Banner(shown.ErrorMessage);
                        }
                        Controls(view, shown);
                    })
                    .Gap(Sdk.SpaceSnug)
                    .Padding(Sdk.SpaceNormal);
            })
            .FillWidth()
            .FillHeight();
    }

    private void Controls(View view, Video playing)
    {
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
                view.IconButton(playing.IsPlaying ? "pause" : "play_arrow")
                    .Tooltip(playing.IsPlaying ? "Pause" : "Play")
                    .Enabled(playing.State != PlaybackState.Failed)
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
                view.IconButton("stop").Tooltip("Stop").OnPress(_ => playing.Stop());
                view.Text($"{Clock(position)} / {Clock(duration)}").TextMonospace();
                view.Spacer();
                view.Icon("volume_up");
                view.Slider(playing.Volume).Width(120).OnValue((_, value) => playing.Volume = value);
                view.Button("Open Video…").ButtonStyle(ButtonVariant.Secondary).OnPress(Choose);
            })
            .Gap(Sdk.SpaceSnug)
            .AlignItems(Align.Center);
    }

    private void Choose(Context context)
    {
        var chooser = new FileChooser(FileChooserPurpose.Open, "Open Video")
            .AddFilter("Videos")
            .AddFilterMimeType("video/*");
        context.ChooseFiles(chooser, (_, status, paths) =>
        {
            if (status != Status.OK)
            {
                return;
            }
            video?.Stop();
            video?.Dispose();
            scrub = null;
            video = new Video(paths[0]);
            video.Play();
        });
    }
}
