import os.rikka.App;
import os.rikka.ButtonVariant;
import os.rikka.Rikka;
import os.rikka.RikkaException;
import os.rikka.TextRole;
import os.rikka.View;
import os.rikka.WindowBuilder;

public final class Counter {
    private long count;

    private void view(View view) {
        view.column(() -> {
                    view.text(Long.toString(count)).textStyle(TextRole.DISPLAY);
                    view.row(() -> {
                                view.button("Add").onPress(context -> count++);
                                view.button("Remove")
                                        .buttonStyle(ButtonVariant.SECONDARY)
                                        .enabled(count > 0)
                                        .onPress(context -> count--);
                            })
                            .gap(Rikka.SPACE_SNUG);
                })
                .gap(Rikka.SPACE_ROOMY)
                .padding(Rikka.SPACE_LOOSE);
    }

    public static void main(String[] arguments) {
        Counter counter = new Counter();
        WindowBuilder window = new WindowBuilder(counter::view).title("Counter").size(360, 240);
        try {
            new App("org.rikka.examples.counter").addWindow(window).run();
        } catch (RikkaException exception) {
            System.err.println("counter: " + exception.getMessage());
            System.exit(1);
        }
    }
}
