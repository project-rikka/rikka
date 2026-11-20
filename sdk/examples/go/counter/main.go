package main

import (
	"fmt"
	"os"
	"strconv"

	rikka "github.com/project-rikka/rikka/sdk/bindings/go"
)

type counter struct {
	count int
}

func (c *counter) view(view *rikka.View) {
	view.Column(func() {
		view.Text(strconv.Itoa(c.count)).TextStyle(rikka.TextRoleDisplay)
		view.Row(func() {
			view.Button("Add").OnPress(func(*rikka.Context) { c.count++ })
			view.Button("Remove").
				ButtonStyle(rikka.ButtonVariantSecondary).
				Enabled(c.count > 0).
				OnPress(func(*rikka.Context) { c.count-- })
		}).Gap(rikka.SpaceSnug)
	}).Gap(rikka.SpaceRoomy).Padding(rikka.SpaceLoose)
}

func main() {
	state := &counter{}
	window := rikka.NewWindowBuilder(state.view).Title("Counter").Size(360, 240)
	if err := rikka.NewApp("org.rikka.examples.counter").AddWindow(window).Run(); err != nil {
		fmt.Fprintf(os.Stderr, "counter: %v\n", err)
		os.Exit(1)
	}
}
