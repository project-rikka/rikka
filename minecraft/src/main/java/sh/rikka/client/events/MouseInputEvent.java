package sh.rikka.client.events;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.input.MouseButtonInfo;
import sh.rikka.client.event.CancellableEvent;

public final class MouseInputEvent extends CancellableEvent {
	private final MouseButtonInfo input;
	private final InputAction action;

	MouseInputEvent(MouseButtonInfo input, InputAction action) {
		this.input = input;
		this.action = action;
	}

	public MouseButtonInfo input() {
		return this.input;
	}

	public InputAction action() {
		return this.action;
	}

	public InputConstants.Key key() {
		return InputConstants.Type.MOUSE.getOrCreate(this.input.button());
	}
}
