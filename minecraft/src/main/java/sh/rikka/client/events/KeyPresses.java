package sh.rikka.client.events;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.Minecraft;
import sh.rikka.client.event.CancellableEvent;
import sh.rikka.client.event.EventHandler;

public final class KeyPresses {
	@FunctionalInterface
	public interface Handler {
		boolean pressed(InputConstants.Key key);
	}

	private final Handler handler;

	public KeyPresses(Handler handler) {
		this.handler = handler;
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
		if (action == InputAction.PRESS && Minecraft.getInstance().gui.screen() == null && this.handler.pressed(key)) {
			event.cancel();
		}
	}
}
