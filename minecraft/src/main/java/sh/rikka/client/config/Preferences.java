package sh.rikka.client.config;

import com.mojang.blaze3d.platform.InputConstants;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.KeybindSetting;
import sh.rikka.client.setting.Settings;
import sh.rikka.client.setting.StringSetting;

public final class Preferences {
	private static final double DEFAULT_MUSIC_VOLUME = 0.7;
	private static final double MAX_MUSIC_POSITION = 24.0 * 60.0 * 60.0;
	private static final int MAX_MUSIC_TRACK_LENGTH = 4096;

	private final Settings settings = new Settings(RikkaClient.MOD_ID + ".preference");
	private final KeybindSetting menuKey = this.settings.keybind("menu_key", InputConstants.KEY_RSHIFT);
	private final KeybindSetting hudEditorKey = this.settings.keybind("hud_editor_key");
	private final StringSetting theme = this.settings.text("theme", "");
	private final EnumSetting<Background> background = this.settings.choice("background", Background.DESKTOP);
	private final KeybindSetting musicPlayKey = this.settings.keybind("music_play_key");
	private final KeybindSetting musicNextKey = this.settings.keybind("music_next_key");
	private final KeybindSetting musicPreviousKey = this.settings.keybind("music_previous_key");
	private final DoubleSetting musicVolume = this.settings.number("music_volume", DEFAULT_MUSIC_VOLUME, 0.0, 1.0);
	private final BoolSetting musicMuted = this.settings.bool("music_muted", false);
	private final BoolSetting musicShuffle = this.settings.bool("music_shuffle", false);
	private final EnumSetting<MusicRepeat> musicRepeat = this.settings.choice("music_repeat", MusicRepeat.OFF);
	private final StringSetting musicTrack = this.settings.text("music_track", "").maxLength(MAX_MUSIC_TRACK_LENGTH);
	private final DoubleSetting musicPosition = this.settings.number("music_position", 0.0, 0.0, MAX_MUSIC_POSITION);

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

	public KeybindSetting musicPlayKey() {
		return this.musicPlayKey;
	}

	public KeybindSetting musicNextKey() {
		return this.musicNextKey;
	}

	public KeybindSetting musicPreviousKey() {
		return this.musicPreviousKey;
	}

	public DoubleSetting musicVolume() {
		return this.musicVolume;
	}

	public BoolSetting musicMuted() {
		return this.musicMuted;
	}

	public BoolSetting musicShuffle() {
		return this.musicShuffle;
	}

	public EnumSetting<MusicRepeat> musicRepeat() {
		return this.musicRepeat;
	}

	public StringSetting musicTrack() {
		return this.musicTrack;
	}

	public DoubleSetting musicPosition() {
		return this.musicPosition;
	}
}
