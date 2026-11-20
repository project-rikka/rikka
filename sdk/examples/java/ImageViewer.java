import java.nio.file.Path;
import os.rikka.Align;
import os.rikka.App;
import os.rikka.Context;
import os.rikka.FileChooser;
import os.rikka.FileChooserPurpose;
import os.rikka.Fit;
import os.rikka.Image;
import os.rikka.Rikka;
import os.rikka.RikkaException;
import os.rikka.Status;
import os.rikka.View;
import os.rikka.WindowBuilder;

public final class ImageViewer {
    private String path;
    private Image image;

    private void view(View view) {
        view.column(() -> {
                    view.row(() -> {
                                view.button("Open Picture…").onPress(this::choose);
                                view.iconButton("open_in_new")
                                        .tooltip("Open in Default App")
                                        .enabled(path != null)
                                        .onPress(this::openElsewhere);
                                String name = path != null ? Path.of(path).getFileName().toString() : "";
                                view.text(name).textNoWrap().grow(1);
                            })
                            .gap(Rikka.SPACE_SNUG)
                            .padding(Rikka.SPACE_NORMAL)
                            .alignItems(Align.CENTER);
                    view.divider();
                    if (image == null) {
                        view.statusPage("image", "Drop a picture here, or open one.", () -> {}).grow(1);
                    } else {
                        view.imageView(image).imageFit(Fit.CONTAIN).fillWidth().grow(1);
                    }
                })
                .fillWidth()
                .fillHeight()
                .onDropFiles((context, paths) -> show(paths.get(0)));
    }

    private void show(String chosen) {
        if (image != null) {
            image.close();
        }
        path = chosen;
        image = Image.fromFile(chosen);
    }

    private void choose(Context context) {
        FileChooser chooser = new FileChooser(FileChooserPurpose.OPEN, "Open Picture")
                .addFilter("Pictures")
                .addFilterMimeType("image/*");
        context.chooseFiles(chooser, (chosen, status, paths) -> {
            if (status == Status.OK) {
                show(paths.get(0));
            }
        });
    }

    private void openElsewhere(Context context) {
        try {
            Rikka.openFile(path);
        } catch (RikkaException exception) {
            context.alert("Couldn’t Open the Picture", exception.getMessage());
        }
    }

    public static void main(String[] arguments) {
        ImageViewer viewer = new ImageViewer();
        WindowBuilder window = new WindowBuilder(viewer::view).title("Image Viewer").size(720, 540);
        try {
            new App("org.rikka.examples.image_viewer").addWindow(window).run();
        } catch (RikkaException exception) {
            System.err.println("image_viewer: " + exception.getMessage());
            System.exit(1);
        }
    }
}
