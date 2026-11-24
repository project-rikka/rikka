package sh.rikka.client.mods.translate;

import java.util.Optional;
import java.util.UUID;
import net.minecraft.client.gui.Font;
import net.minecraft.client.multiplayer.chat.GuiMessage;
import net.minecraft.network.chat.ClickEvent;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

public final class ChatTranslateHooks {
	private ChatTranslateHooks() {
	}

	public static void body(Component decorated, Component content, Optional<UUID> sender) {
		ChatTranslateMod mod = ChatTranslateMod.active();
		if (mod != null) {
			mod.incoming().body(decorated, content, sender);
		}
	}

	public static GuiMessage decorate(GuiMessage message) {
		ChatTranslateMod mod = ChatTranslateMod.active();
		return mod == null ? message : mod.incoming().decorate(message);
	}

	public static boolean click(@Nullable ClickEvent event) {
		if (!(event instanceof ClickEvent.Custom custom) || !custom.id().equals(IncomingTranslations.BUTTON)) {
			return false;
		}
		ChatTranslateMod mod = ChatTranslateMod.active();
		if (mod != null) {
			mod.incoming().click(custom.payload());
		}
		return true;
	}

	public static boolean send(String message, boolean addToRecent) {
		ChatTranslateMod mod = ChatTranslateMod.active();
		return mod != null && mod.sending().send(message, addToRecent);
	}

	public static Optional<TranslateBar> bar(Font font, int screenWidth, int screenHeight) {
		ChatTranslateMod mod = ChatTranslateMod.active();
		return mod == null ? Optional.empty() : Optional.of(new TranslateBar(mod, font, screenWidth, screenHeight));
	}
}
