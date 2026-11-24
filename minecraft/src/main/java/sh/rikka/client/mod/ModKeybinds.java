package sh.rikka.client.mod;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.Minecraft;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.InputAction;
import sh.rikka.client.events.KeyInputEvent;
import sh.rikka.client.events.MouseInputEvent;

public final class ModKeybinds {
	private final ModManager mods;

	public ModKeybinds(ModManager mods) {
		this.mods = mods;
	}

	@EventHandler
	private void onKeyInput(KeyInputEvent event) {
		handle(event.key(), event.action());
	}

	@EventHandler
	private void onMouseInput(MouseInputEvent event) {
		handle(event.key(), event.action());
	}

	private void handle(InputConstants.Key key, InputAction action) {
		if (action == InputAction.REPEAT) {
			return;
		}
		boolean inGame = Minecraft.getInstance().gui.screen() == null;
		for (Mod mod : this.mods.all()) {
			if (!mod.bind().matches(key)) {
				continue;
			}
			switch (mod.bindMode().get()) {
				case TOGGLE -> {
					if (action == InputAction.PRESS && inGame) {
						mod.toggle();
					}
				}
				case HOLD -> {
					if (action == InputAction.PRESS && inGame) {
						mod.setEnabled(true);
					} else if (action == InputAction.RELEASE) {
						mod.setEnabled(false);
					}
				}
			}
		}
	}
}
