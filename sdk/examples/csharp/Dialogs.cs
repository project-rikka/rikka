using System;
using Rikka;

const string Website = "https://github.com/project-rikka/rikka";

var answer = "Nothing was deleted yet.";

var window = new WindowBuilder(view =>
    {
        view.Column(() =>
            {
                view.Text("Dialogs").TextStyle(TextRole.Title);
                view.Button("Show a Message")
                    .OnPress(context => context.Alert("Hello", "This message was shown with Context.Alert."));
                view.Row(() =>
                    {
                        view.Button("Delete Everything…")
                            .ButtonStyle(ButtonVariant.Danger)
                            .OnPress(context => context.Confirm(
                                "Delete everything?",
                                "This cannot be undone.",
                                "Delete",
                                (_, confirmed) => answer = confirmed ? "Everything was deleted." : "You kept everything."));
                        view.Text(answer).Tone(TextTone.Muted);
                    })
                    .Gap(Sdk.SpaceNormal)
                    .AlignItems(Align.Center);
                view.Button("Visit the Website")
                    .ButtonStyle(ButtonVariant.Secondary)
                    .OnPress(context =>
                    {
                        try
                        {
                            Sdk.OpenUrl(Website);
                        }
                        catch (RikkaException exception)
                        {
                            context.Alert("Couldn’t Open the Website", exception.Message);
                        }
                    });
            })
            .Gap(Sdk.SpaceRoomy)
            .Padding(Sdk.SpaceLoose);
    })
    .SetTitle("Dialogs")
    .SetSize(480, 320);

try
{
    new App("org.rikka.examples.dialogs").AddWindow(window).Run();
}
catch (RikkaException exception)
{
    Console.Error.WriteLine($"dialogs: {exception.Message}");
    return 1;
}
return 0;
