import os.rikka.Align;
import os.rikka.App;
import os.rikka.ButtonVariant;
import os.rikka.Context;
import os.rikka.Rikka;
import os.rikka.RikkaException;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.View;
import os.rikka.WindowBuilder;

public final class Dialogs {
    private static final String WEBSITE = "https://github.com/project-rikka/rikka";

    private String answer = "Nothing was deleted yet.";

    private void view(View view) {
        view.column(() -> {
                    view.text("Dialogs").textStyle(TextRole.TITLE);
                    view.button("Show a Message")
                            .onPress(context -> context.alert("Hello", "This message was shown with Context.alert."));
                    view.row(() -> {
                                view.button("Delete Everything…")
                                        .buttonStyle(ButtonVariant.DANGER)
                                        .onPress(this::askToDelete);
                                view.text(answer).tone(TextTone.MUTED);
                            })
                            .gap(Rikka.SPACE_NORMAL)
                            .alignItems(Align.CENTER);
                    view.button("Visit the Website")
                            .buttonStyle(ButtonVariant.SECONDARY)
                            .onPress(this::visit);
                })
                .gap(Rikka.SPACE_ROOMY)
                .padding(Rikka.SPACE_LOOSE);
    }

    private void askToDelete(Context context) {
        context.confirm("Delete everything?", "This cannot be undone.", "Delete", (answered, confirmed) ->
                answer = confirmed ? "Everything was deleted." : "You kept everything.");
    }

    private void visit(Context context) {
        try {
            Rikka.openUrl(WEBSITE);
        } catch (RikkaException exception) {
            context.alert("Couldn’t Open the Website", exception.getMessage());
        }
    }

    public static void main(String[] arguments) {
        Dialogs dialogs = new Dialogs();
        WindowBuilder window = new WindowBuilder(dialogs::view).title("Dialogs").size(480, 320);
        try {
            new App("org.rikka.examples.dialogs").addWindow(window).run();
        } catch (RikkaException exception) {
            System.err.println("dialogs: " + exception.getMessage());
            System.exit(1);
        }
    }
}
