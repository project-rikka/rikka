package sh.rikka.client.mods.hud;

import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.ScreenPosition;

public final class WeatherDisplayMod extends SimpleHudMod {
	public WeatherDisplayMod(ScreenPosition defaultPosition) {
		super("weather_display", defaultPosition, Weather.NONE.icon());
	}

	@Override
	protected String label() {
		@Nullable Weather weather = current();
		return weather == null ? UNAVAILABLE : translate(weather.translationSuffix());
	}

	@Override
	protected String shownIcon() {
		@Nullable Weather weather = current();
		return weather == null ? icon() : weather.icon();
	}

	private static @Nullable Weather current() {
		Minecraft minecraft = Minecraft.getInstance();
		ClientLevel level = minecraft.level;
		LocalPlayer player = minecraft.player;
		return level == null || player == null ? null : Weather.at(level, player.blockPosition());
	}
}
