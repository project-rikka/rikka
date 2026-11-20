package main

import (
	"fmt"
	"os"

	rikka "github.com/project-rikka/rikka/sdk/bindings/go"
)

const website = "https://github.com/project-rikka/rikka"

type dialogs struct {
	answer string
}

func (d *dialogs) view(view *rikka.View) {
	view.Column(func() {
		view.Text("Dialogs").TextStyle(rikka.TextRoleTitle)
		view.Button("Show a Message").OnPress(func(context *rikka.Context) {
			context.Alert("Hello", "This message was shown with Context.Alert.")
		})
		view.Row(func() {
			view.Button("Delete Everything…").
				ButtonStyle(rikka.ButtonVariantDanger).
				OnPress(d.askToDelete)
			view.Text(d.answer).Tone(rikka.TextToneMuted)
		}).Gap(rikka.SpaceNormal).AlignItems(rikka.AlignCenter)
		view.Button("Visit the Website").
			ButtonStyle(rikka.ButtonVariantSecondary).
			OnPress(func(context *rikka.Context) {
				if err := rikka.OpenURL(website); err != nil {
					context.Alert("Couldn’t Open the Website", err.Error())
				}
			})
	}).Gap(rikka.SpaceRoomy).Padding(rikka.SpaceLoose)
}

func (d *dialogs) askToDelete(context *rikka.Context) {
	context.Confirm("Delete everything?", "This cannot be undone.", "Delete",
		func(_ *rikka.Context, confirmed bool) {
			if confirmed {
				d.answer = "Everything was deleted."
			} else {
				d.answer = "You kept everything."
			}
		})
}

func main() {
	state := &dialogs{answer: "Nothing was deleted yet."}
	window := rikka.NewWindowBuilder(state.view).Title("Dialogs").Size(480, 320)
	if err := rikka.NewApp("org.rikka.examples.dialogs").AddWindow(window).Run(); err != nil {
		fmt.Fprintf(os.Stderr, "dialogs: %v\n", err)
		os.Exit(1)
	}
}
