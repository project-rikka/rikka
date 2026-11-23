package sh.rikka.client.mod;

import com.mojang.blaze3d.platform.InputConstants;

public record DefaultBind(InputConstants.Key key, BindMode mode) {
	public static final DefaultBind NONE = new DefaultBind(InputConstants.UNKNOWN, BindMode.TOGGLE);

	public static DefaultBind hold(int keyboardKey) {
		return new DefaultBind(InputConstants.Type.KEYBOARD.getOrCreate(keyboardKey), BindMode.HOLD);
	}
}
