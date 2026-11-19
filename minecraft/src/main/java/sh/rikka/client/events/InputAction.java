package sh.rikka.client.events;

import com.mojang.blaze3d.platform.InputConstants;

public enum InputAction {
	PRESS,
	RELEASE,
	REPEAT;

	static InputAction of(int action) {
		return switch (action) {
			case InputConstants.PRESS -> PRESS;
			case InputConstants.RELEASE -> RELEASE;
			case InputConstants.REPEAT -> REPEAT;
			default -> throw new IllegalArgumentException("unknown input action: " + action);
		};
	}
}
