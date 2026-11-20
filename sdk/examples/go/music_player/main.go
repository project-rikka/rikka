package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	rikka "github.com/project-rikka/rikka/sdk/bindings/go"
)

func clock(seconds float64) string {
	whole := int(seconds)
	return fmt.Sprintf("%d:%02d", whole/60, whole%60)
}

type musicPlayer struct {
	tracks  []string
	current int
	sound   *rikka.Sound
	scrub   *float64
}

func (p *musicPlayer) view(view *rikka.View) {
	view.Column(func() {
		view.Row(func() {
			view.Text("Music").TextStyle(rikka.TextRoleTitle).Grow(1)
			view.Button("Choose Music…").OnPress(p.choose)
		}).Gap(rikka.SpaceSnug).AlignItems(rikka.AlignCenter)
		if len(p.tracks) == 0 {
			view.Text("Choose one or more music files to play them in order.").Tone(rikka.TextToneMuted)
			return
		}
		view.List(func() {
			for _, track := range p.tracks {
				name := strings.TrimSuffix(filepath.Base(track), filepath.Ext(track))
				view.ListItem(name).LeadingIcon("music_note")
			}
		}).Selection(p.current).OnSelect(func(context *rikka.Context, index int) {
			p.start(context, index)
		}).Grow(1)
		if p.sound != nil {
			p.controls(view, p.sound)
		}
	}).Gap(rikka.SpaceRoomy).Padding(rikka.SpaceLoose).FillWidth().FillHeight()
}

func (p *musicPlayer) controls(view *rikka.View, sound *rikka.Sound) {
	if sound.State() == rikka.PlaybackStateFailed {
		view.Banner(sound.ErrorMessage())
		return
	}
	duration := sound.Duration()
	position := sound.Position()
	if p.scrub != nil {
		position = *p.scrub
	}
	view.Slider(position).
		Range(0, max(duration, 1)).
		Enabled(duration > 0).
		OnValue(func(_ *rikka.Context, value float64) { p.scrub = &value }).
		OnValueCommit(func(_ *rikka.Context, value float64) {
			p.scrub = nil
			sound.Seek(value)
		})
	view.Row(func() {
		view.IconButton("skip_previous").
			Tooltip("Previous").
			Enabled(p.current > 0).
			OnPress(func(context *rikka.Context) { p.start(context, p.current-1) })
		icon, label := "play_arrow", "Play"
		if sound.IsPlaying() {
			icon, label = "pause", "Pause"
		}
		view.IconButton(icon).Tooltip(label).OnPress(func(*rikka.Context) {
			if sound.IsPlaying() {
				sound.Pause()
			} else {
				sound.Play()
			}
		})
		view.IconButton("skip_next").
			Tooltip("Next").
			Enabled(p.current+1 < len(p.tracks)).
			OnPress(p.next)
		view.Text(clock(position) + " / " + clock(duration)).TextMonospace()
		view.Spacer()
		repeat := rikka.CheckStateUnchecked
		if sound.Looping() {
			repeat = rikka.CheckStateChecked
		}
		view.Checkbox(repeat, "Repeat").OnToggle(func(_ *rikka.Context, on bool) { sound.SetLooping(on) })
		view.Icon("volume_up")
		view.Slider(sound.Volume()).
			Width(120).
			OnValue(func(_ *rikka.Context, value float64) { sound.SetVolume(value) })
	}).Gap(rikka.SpaceSnug).AlignItems(rikka.AlignCenter)
}

func (p *musicPlayer) choose(context *rikka.Context) {
	chooser := rikka.NewFileChooser(rikka.FileChooserPurposeOpen, "Choose Music").
		Multiple(true).
		AddFilter("Music").
		AddFilterMIMEType("audio/*")
	context.ChooseFiles(chooser, func(context *rikka.Context, status error, paths []string) {
		if status == nil {
			p.tracks = paths
			p.start(context, 0)
		}
	})
}

func (p *musicPlayer) start(context *rikka.Context, index int) {
	if p.sound != nil {
		p.sound.Stop()
	}
	p.current = index
	p.scrub = nil
	p.sound = rikka.NewSound(p.tracks[index])
	p.sound.OnEnd(context.Window(), p.next)
	p.sound.Play()
}

func (p *musicPlayer) next(context *rikka.Context) {
	if p.current+1 < len(p.tracks) {
		p.start(context, p.current+1)
	}
}

func main() {
	player := &musicPlayer{}
	window := rikka.NewWindowBuilder(player.view).Title("Music Player").Size(560, 520)
	if err := rikka.NewApp("org.rikka.examples.music_player").AddWindow(window).Run(); err != nil {
		fmt.Fprintf(os.Stderr, "music_player: %v\n", err)
		os.Exit(1)
	}
}
