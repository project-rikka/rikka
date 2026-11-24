package sh.rikka.client.config;

import com.mojang.blaze3d.platform.InputConstants;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.KeybindSetting;
import sh.rikka.client.setting.Settings;
import sh.rikka.client.setting.StringSetting;

public final class Preferences {
	private final Settings settings = new Settings(RikkaClient.MOD_ID + ".preference");
	private final KeybindSetting menuKey = this.settings.keybind("menu_key", InputConstants.KEY_RSHIFT);
	private final KeybindSetting hudEditorKey = this.settings.keybind("hud_editor_key");
	private final StringSetting theme = this.settings.text("theme", "");
	private final EnumSetting<Background> background = this.settings.choice("background", Background.DESKTOP);

	public Preferences() {
		this.settings.lock();
	}

	public Settings settings() {
		return this.settings;
	}

	public KeybindSetting menuKey() {
		return this.menuKey;
	}

	public KeybindSetting hudEditorKey() {
		return this.hudEditorKey;
	}

	public StringSetting theme() {
		return this.theme;
	}

	public EnumSetting<Background> background() {
		return this.background;
	}
}
