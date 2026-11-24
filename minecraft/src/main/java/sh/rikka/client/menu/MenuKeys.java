package sh.rikka.client.menu;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.Minecraft;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.config.Preferences;
import sh.rikka.client.event.CancellableEvent;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.InputAction;
import sh.rikka.client.events.KeyInputEvent;
import sh.rikka.client.events.MouseInputEvent;
import sh.rikka.client.hud.HudEditorScreen;

public final class MenuKeys {
	private final Preferences preferences;
	private @Nullable MenuState state;

	public MenuKeys(Preferences preferences) {
		this.preferences = preferences;
	}

	@EventHandler
	private void onKeyInput(KeyInputEvent event) {
		handle(event.key(), event.action(), event);
	}

	@EventHandler
	private void onMouseInput(MouseInputEvent event) {
		handle(event.key(), event.action(), event);
	}

	private void handle(InputConstants.Key key, InputAction action, CancellableEvent event) {
		Minecraft minecraft = Minecraft.getInstance();
		if (action != InputAction.PRESS || minecraft.gui.screen() != null) {
			return;
		}
		if (this.preferences.menuKey().matches(key)) {
			minecraft.gui.setScreen(new ModMenuScreen(state()));
			event.cancel();
		} else if (this.preferences.hudEditorKey().matches(key) && minecraft.level != null) {
			minecraft.gui.setScreen(HudEditorScreen.standalone(state()));
			event.cancel();
		}
	}

	private MenuState state() {
		MenuState current = this.state;
		if (current == null) {
			current = new MenuState();
			this.state = current;
		}
		return current;
	}
}
