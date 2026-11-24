package sh.rikka.client.events;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.input.KeyEvent;
import sh.rikka.client.event.CancellableEvent;

public final class KeyInputEvent extends CancellableEvent {
	private final KeyEvent input;
	private final InputAction action;

	KeyInputEvent(KeyEvent input, InputAction action) {
		this.input = input;
		this.action = action;
	}

	public KeyEvent input() {
		return this.input;
	}

	public InputAction action() {
		return this.action;
	}

	public InputConstants.Key key() {
		return InputConstants.getKey(this.input);
	}
}
