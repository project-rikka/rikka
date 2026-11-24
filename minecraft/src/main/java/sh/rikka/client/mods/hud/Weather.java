package sh.rikka.client.mods.hud;

import java.util.Locale;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;

enum Weather {
	CLEAR("sunny"),
	RAIN("rainy"),
	SNOW("weather_snowy"),
	THUNDER("thunderstorm"),
	NONE("cloud_off");

	private final String icon;
	private final String translationSuffix;

	Weather(String icon) {
		this.icon = icon;
		this.translationSuffix = "weather." + name().toLowerCase(Locale.ROOT);
	}

	static Weather at(ClientLevel level, BlockPos position) {
		if (!level.canHaveWeather()) {
			return NONE;
		}
		if (!level.isRaining()) {
			return CLEAR;
		}
		return switch (level.getPrecipitationAt(position)) {
			case NONE -> CLEAR;
			case RAIN -> level.isThundering() ? THUNDER : RAIN;
			case SNOW -> level.isThundering() ? THUNDER : SNOW;
		};
	}

	String icon() {
		return this.icon;
	}

	String translationSuffix() {
		return this.translationSuffix;
	}
}
