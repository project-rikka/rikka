package main

import (
	"fmt"
	"os"

	rikka "github.com/project-rikka/rikka/sdk/bindings/go"
)

var black = rikka.Color{R: 0, G: 0, B: 0, A: 1}

func clock(seconds float64) string {
	whole := int(seconds)
	return fmt.Sprintf("%d:%02d", whole/60, whole%60)
}

type videoPlayer struct {
	video *rikka.Video
	scrub *float64
}

func (p *videoPlayer) view(view *rikka.View) {
	video := p.video
	if video == nil {
		view.StatusPage("movie", "Open a video to play it.", func() {
			view.Button("Open Video…").OnPress(p.choose)
		}).FillWidth().FillHeight()
		return
	}
	view.Column(func() {
		view.VideoView(video).ImageFit(rikka.FitContain).Background(black).FillWidth().Grow(1)
		view.Column(func() {
			if message := video.ErrorMessage(); message != "" {
				view.Banner(message)
			}
			p.controls(view, video)
		}).Gap(rikka.SpaceSnug).Padding(rikka.SpaceNormal)
	}).FillWidth().FillHeight()
}

func (p *videoPlayer) controls(view *rikka.View, video *rikka.Video) {
	duration := video.Duration()
	position := video.Position()
	if p.scrub != nil {
		position = *p.scrub
	}
	view.Slider(position).
		Range(0, max(duration, 1)).
		Enabled(duration > 0).
		OnValue(func(_ *rikka.Context, value float64) { p.scrub = &value }).
		OnValueCommit(func(_ *rikka.Context, value float64) {
			p.scrub = nil
			video.Seek(value)
		})
	view.Row(func() {
		icon, label := "play_arrow", "Play"
		if video.IsPlaying() {
			icon, label = "pause", "Pause"
		}
		view.IconButton(icon).
			Tooltip(label).
			Enabled(video.State() != rikka.PlaybackStateFailed).
			OnPress(func(*rikka.Context) {
				if video.IsPlaying() {
					video.Pause()
				} else {
					video.Play()
				}
			})
		view.IconButton("stop").Tooltip("Stop").OnPress(func(*rikka.Context) { video.Stop() })
		view.Text(clock(position) + " / " + clock(duration)).TextMonospace()
		view.Spacer()
		view.Icon("volume_up")
		view.Slider(video.Volume()).
			Width(120).
			OnValue(func(_ *rikka.Context, value float64) { video.SetVolume(value) })
		view.Button("Open Video…").ButtonStyle(rikka.ButtonVariantSecondary).OnPress(p.choose)
	}).Gap(rikka.SpaceSnug).AlignItems(rikka.AlignCenter)
}

func (p *videoPlayer) choose(context *rikka.Context) {
	chooser := rikka.NewFileChooser(rikka.FileChooserPurposeOpen, "Open Video").
		AddFilter("Videos").
		AddFilterMIMEType("video/*")
	context.ChooseFiles(chooser, func(_ *rikka.Context, status error, paths []string) {
		if status != nil {
			return
		}
		if p.video != nil {
			p.video.Stop()
		}
		p.scrub = nil
		p.video = rikka.NewVideo(paths[0])
		p.video.Play()
	})
}

func main() {
	player := &videoPlayer{}
	window := rikka.NewWindowBuilder(player.view).Title("Video Player").Size(800, 560)
	if err := rikka.NewApp("org.rikka.examples.video_player").AddWindow(window).Run(); err != nil {
		fmt.Fprintf(os.Stderr, "video_player: %v\n", err)
		os.Exit(1)
	}
}
