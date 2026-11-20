package main

import (
	"fmt"
	"os"
	"path/filepath"

	rikka "github.com/project-rikka/rikka/sdk/bindings/go"
)

type imageViewer struct {
	path  string
	image *rikka.Image
}

func (v *imageViewer) view(view *rikka.View) {
	view.Column(func() {
		view.Row(func() {
			view.Button("Open Picture…").OnPress(v.choose)
			view.IconButton("open_in_new").
				Tooltip("Open in Default App").
				Enabled(v.path != "").
				OnPress(v.openElsewhere)
			name := ""
			if v.path != "" {
				name = filepath.Base(v.path)
			}
			view.Text(name).TextNoWrap().Grow(1)
		}).Gap(rikka.SpaceSnug).Padding(rikka.SpaceNormal).AlignItems(rikka.AlignCenter)
		view.Divider()
		if v.image == nil {
			view.StatusPage("image", "Drop a picture here, or open one.", func() {}).Grow(1)
		} else {
			view.ImageView(v.image).ImageFit(rikka.FitContain).FillWidth().Grow(1)
		}
	}).FillWidth().FillHeight().OnDropFiles(func(_ *rikka.Context, paths []string) {
		v.show(paths[0])
	})
}

func (v *imageViewer) show(path string) {
	v.path = path
	v.image = rikka.NewImageFromFile(path)
}

func (v *imageViewer) choose(context *rikka.Context) {
	chooser := rikka.NewFileChooser(rikka.FileChooserPurposeOpen, "Open Picture").
		AddFilter("Pictures").
		AddFilterMIMEType("image/*")
	context.ChooseFiles(chooser, func(_ *rikka.Context, status error, paths []string) {
		if status == nil {
			v.show(paths[0])
		}
	})
}

func (v *imageViewer) openElsewhere(context *rikka.Context) {
	if err := rikka.OpenFile(v.path); err != nil {
		context.Alert("Couldn’t Open the Picture", err.Error())
	}
}

func main() {
	viewer := &imageViewer{}
	window := rikka.NewWindowBuilder(viewer.view).Title("Image Viewer").Size(720, 540)
	if err := rikka.NewApp("org.rikka.examples.image_viewer").AddWindow(window).Run(); err != nil {
		fmt.Fprintf(os.Stderr, "image_viewer: %v\n", err)
		os.Exit(1)
	}
}
