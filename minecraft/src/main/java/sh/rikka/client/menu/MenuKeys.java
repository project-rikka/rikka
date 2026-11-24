package sh.rikka.client.menu;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.Minecraft;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.config.Preferences;
import sh.rikka.client.events.KeyPresses;
import sh.rikka.client.hud.HudEditorScreen;

public final class MenuKeys implements KeyPresses.Handler {
	private final Preferences preferences;
	private @Nullable MenuState state;

	public MenuKeys(Preferences preferences) {
		this.preferences = preferences;
	}

	@Override
	public boolean pressed(InputConstants.Key key) {
		Minecraft minecraft = Minecraft.getInstance();
		if (this.preferences.menuKey().matches(key)) {
			minecraft.gui.setScreen(new ModMenuScreen(state()));
			return true;
		}
		if (this.preferences.hudEditorKey().matches(key) && minecraft.level != null) {
			minecraft.gui.setScreen(HudEditorScreen.standalone(state()));
			return true;
		}
		return false;
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
