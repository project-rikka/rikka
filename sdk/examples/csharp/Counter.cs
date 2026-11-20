using System;
using System.Globalization;
using Rikka;

var count = 0;

var window = new WindowBuilder(view =>
    {
        view.Column(() =>
            {
                view.Text(count.ToString(CultureInfo.CurrentCulture)).TextStyle(TextRole.Display);
                view.Row(() =>
                    {
                        view.Button("Add").OnPress(_ => count++);
                        view.Button("Remove")
                            .ButtonStyle(ButtonVariant.Secondary)
                            .Enabled(count > 0)
                            .OnPress(_ => count--);
                    })
                    .Gap(Sdk.SpaceSnug);
            })
            .Gap(Sdk.SpaceRoomy)
            .Padding(Sdk.SpaceLoose);
    })
    .SetTitle("Counter")
    .SetSize(360, 240);

try
{
    new App("org.rikka.examples.counter").AddWindow(window).Run();
}
catch (RikkaException exception)
{
    Console.Error.WriteLine($"counter: {exception.Message}");
    return 1;
}
return 0;
