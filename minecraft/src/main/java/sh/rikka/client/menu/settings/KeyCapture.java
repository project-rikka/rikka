package sh.rikka.client.menu.settings;

import com.mojang.blaze3d.platform.InputConstants;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.setting.KeybindSetting;

public final class KeyCapture {
	private @Nullable KeybindSetting target;

	public boolean isActive() {
		return this.target != null;
	}

	boolean captures(KeybindSetting setting) {
		return this.target == setting;
	}

	void toggle(KeybindSetting setting) {
		this.target = this.target == setting ? null : setting;
	}

	public void cancel() {
		this.target = null;
	}

	public void accept(InputConstants.Key key) {
		KeybindSetting captured = this.target;
		if (captured == null) {
			throw new IllegalStateException("no keybind is being captured");
		}
		this.target = null;
		captured.set(key);
	}
}
