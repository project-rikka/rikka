package sh.rikka.client.mods.weather;

import net.minecraft.world.level.biome.Biome;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.IntSetting;

public final class WeatherChangerMod extends Mod {
	private static final double MIN_INTENSITY = 0.05;
	private static final double INTENSITY_STEP = 0.05;
	private static final int DEFAULT_STRIKES_PER_MINUTE = 6;
	private static final int MAX_STRIKES_PER_MINUTE = 30;
	private static volatile @Nullable WeatherChangerMod active;

	private final EnumSetting<WeatherType> weather = choice("weather", WeatherType.RAIN);
	private final DoubleSetting rainIntensity = number("rain_intensity", 1.0, MIN_INTENSITY, 1.0)
		.step(INTENSITY_STEP)
		.visibleWhen(() -> this.weather.is(WeatherType.RAIN) || this.weather.is(WeatherType.THUNDER));
	private final DoubleSetting snowIntensity = number("snow_intensity", 1.0, MIN_INTENSITY, 1.0)
		.step(INTENSITY_STEP)
		.visibleWhen(() -> this.weather.is(WeatherType.SNOW));
	private final DoubleSetting thunderIntensity = number("thunder_intensity", 1.0, MIN_INTENSITY, 1.0)
		.step(INTENSITY_STEP)
		.visibleWhen(() -> this.weather.is(WeatherType.THUNDER));
	private final BoolSetting lightning = bool("lightning", true).visibleWhen(() -> this.weather.is(WeatherType.THUNDER));
	private final IntSetting lightningFrequency = integer("lightning_frequency", DEFAULT_STRIKES_PER_MINUTE, 1, MAX_STRIKES_PER_MINUTE)
		.visibleWhen(() -> this.weather.is(WeatherType.THUNDER) && this.lightning.get());

	public WeatherChangerMod() {
		super("weather_changer", Category.VISUAL, "partly_cloudy_day");
	}

	static @Nullable WeatherChangerMod active() {
		return active;
	}

	@Override
	protected void onEnable() {
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
	}

	WeatherTarget target() {
		return switch (this.weather.get()) {
			case CLEAR -> new WeatherTarget(0.0F, 0.0F, null, 0);
			case RAIN -> new WeatherTarget(this.rainIntensity.getFloat(), 0.0F, Biome.Precipitation.RAIN, 0);
			case THUNDER -> {
				float rain = this.rainIntensity.getFloat();
				int strikes = this.lightning.get() ? this.lightningFrequency.get() : 0;
				yield new WeatherTarget(rain, this.thunderIntensity.getFloat() * rain, Biome.Precipitation.RAIN, strikes);
			}
			case SNOW -> new WeatherTarget(this.snowIntensity.getFloat(), 0.0F, Biome.Precipitation.SNOW, 0);
		};
	}
}
