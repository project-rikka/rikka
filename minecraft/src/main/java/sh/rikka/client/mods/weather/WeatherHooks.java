package sh.rikka.client.mods.weather;

import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.biome.Biome;
import sh.rikka.client.mixin.LevelWeatherAccessor;

public final class WeatherHooks {
	private static final WeatherBlend BLEND = new WeatherBlend();
	private static final LightningStrikes STRIKES = new LightningStrikes();

	private WeatherHooks() {
	}

	public static void tick(ClientLevel level) {
		if (!level.tickRateManager().runsNormally()) {
			return;
		}
		WeatherChangerMod mod = WeatherChangerMod.active();
		WeatherTarget target = mod == null ? null : mod.target();
		LevelWeatherAccessor server = (LevelWeatherAccessor) level;
		float serverRain = server.rikka$rainLevel();
		BLEND.tick(level, target, serverRain, server.rikka$thunderLevel() * serverRain);
		if (target != null) {
			STRIKES.tick(level, target.lightningPerMinute(), BLEND.currentThunder(level));
		}
	}

	public static float rainLevel(Level level, float partialTick, float vanilla) {
		return BLEND.rain(level, partialTick, vanilla);
	}

	public static float thunderLevel(Level level, float partialTick, float vanilla) {
		return BLEND.thunder(level, partialTick, vanilla);
	}

	public static Biome.Precipitation precipitation(ClientLevel level, Biome.Precipitation vanilla) {
		Biome.Precipitation forced = BLEND.precipitation(level);
		return forced == null ? vanilla : forced;
	}
}
