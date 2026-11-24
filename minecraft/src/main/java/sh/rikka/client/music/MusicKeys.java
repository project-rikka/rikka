package sh.rikka.client.music;

import com.mojang.blaze3d.platform.InputConstants;
import sh.rikka.client.config.Preferences;
import sh.rikka.client.events.KeyPresses;

public final class MusicKeys implements KeyPresses.Handler {
	private final Preferences preferences;
	private final MusicPlayer player;

	public MusicKeys(Preferences preferences, MusicPlayer player) {
		this.preferences = preferences;
		this.player = player;
	}

	@Override
	public boolean pressed(InputConstants.Key key) {
		if (this.preferences.musicPlayKey().matches(key)) {
			this.player.toggle();
		} else if (this.preferences.musicNextKey().matches(key)) {
			this.player.next();
		} else if (this.preferences.musicPreviousKey().matches(key)) {
			this.player.previous();
		} else {
			return false;
		}
		return true;
	}
}
