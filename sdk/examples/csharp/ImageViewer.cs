using System;
using System.IO;
using Rikka;

var viewer = new ImageViewer();
var window = new WindowBuilder(viewer.Declare).SetTitle("Image Viewer").SetSize(720, 540);

try
{
    new App("org.rikka.examples.image_viewer").AddWindow(window).Run();
}
catch (RikkaException exception)
{
    Console.Error.WriteLine($"image_viewer: {exception.Message}");
    return 1;
}
return 0;

sealed class ImageViewer
{
    private string? path;
    private Image? image;

    public void Declare(View view)
    {
        view.Column(() =>
            {
                view.Row(() =>
                    {
                        view.Button("Open Picture…").OnPress(Choose);
                        view.IconButton("open_in_new")
                            .Tooltip("Open in Default App")
                            .Enabled(path is not null)
                            .OnPress(OpenElsewhere);
                        view.Text(Path.GetFileName(path) ?? "").TextNoWrap().Grow(1);
                    })
                    .Gap(Sdk.SpaceSnug)
                    .Padding(Sdk.SpaceNormal)
                    .AlignItems(Align.Center);
                view.Divider();
                if (image is null)
                {
                    view.StatusPage("image", "Drop a picture here, or open one.", () => { }).Grow(1);
                }
                else
                {
                    view.ImageView(image).ImageFit(Fit.Contain).FillWidth().Grow(1);
                }
            })
            .FillWidth()
            .FillHeight()
            .OnDropFiles((_, paths) => Show(paths[0]));
    }

    private void Show(string chosen)
    {
        image?.Dispose();
        path = chosen;
        image = Image.FromFile(chosen);
    }

    private void Choose(Context context)
    {
        var chooser = new FileChooser(FileChooserPurpose.Open, "Open Picture")
            .AddFilter("Pictures")
            .AddFilterMimeType("image/*");
        context.ChooseFiles(chooser, (_, status, paths) =>
        {
            if (status == Status.OK)
            {
                Show(paths[0]);
            }
        });
    }

    private void OpenElsewhere(Context context)
    {
        if (path is null)
        {
            return;
        }
        try
        {
            Sdk.OpenFile(path);
        }
        catch (RikkaException exception)
        {
            context.Alert("Couldn’t Open the Picture", exception.Message);
        }
    }
}
