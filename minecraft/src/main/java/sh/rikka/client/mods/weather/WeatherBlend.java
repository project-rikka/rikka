package sh.rikka.client.mods.weather;

import java.lang.ref.WeakReference;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.util.Mth;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.biome.Biome;
import org.jspecify.annotations.Nullable;

final class WeatherBlend {
	private static final float STEP_PER_TICK = 0.01F;

	private @Nullable WeakReference<Level> level;
	private boolean engaged;
	private float previousRain;
	private float rain;
	private float previousThunder;
	private float thunder;
	private Biome.@Nullable Precipitation precipitation;

	void tick(ClientLevel level, @Nullable WeatherTarget target, float serverRain, float serverThunder) {
		boolean switched = !tracks(level);
		if (switched) {
			this.level = new WeakReference<>(level);
			this.engaged = false;
			this.precipitation = null;
		}
		if (!level.canHaveWeather()) {
			this.engaged = false;
			return;
		}
		if (target == null) {
			fadeToServer(serverRain, serverThunder);
			return;
		}
		if (!this.engaged) {
			this.engaged = true;
			if (switched) {
				snap(target.rain(), target.thunder());
			} else {
				snap(serverRain, serverThunder);
			}
		}
		if (target.precipitation() != null) {
			this.precipitation = target.precipitation();
		}
		step(target.rain(), target.thunder());
	}

	float rain(Level level, float partialTick, float vanilla) {
		return applies(level) ? Mth.lerp(partialTick, this.previousRain, this.rain) : vanilla;
	}

	float thunder(Level level, float partialTick, float vanilla) {
		return applies(level) ? Mth.lerp(partialTick, this.previousThunder, this.thunder) : vanilla;
	}

	float currentThunder(Level level) {
		return applies(level) ? this.thunder : 0.0F;
	}

	Biome.@Nullable Precipitation precipitation(Level level) {
		return applies(level) ? this.precipitation : null;
	}

	private void fadeToServer(float serverRain, float serverThunder) {
		if (!this.engaged) {
			return;
		}
		step(serverRain, serverThunder);
		if (this.rain == serverRain && this.thunder == serverThunder) {
			this.engaged = false;
			this.precipitation = null;
		}
	}

	private void snap(float rain, float thunder) {
		this.previousRain = rain;
		this.rain = rain;
		this.previousThunder = thunder;
		this.thunder = thunder;
	}

	private void step(float targetRain, float targetThunder) {
		this.previousRain = this.rain;
		this.rain = approach(this.rain, targetRain);
		this.previousThunder = this.thunder;
		this.thunder = approach(this.thunder, targetThunder);
	}

	private boolean applies(Level level) {
		return this.engaged && tracks(level);
	}

	private boolean tracks(Level level) {
		return this.level != null && this.level.get() == level;
	}

	private static float approach(float current, float target) {
		return current < target ? Math.min(current + STEP_PER_TICK, target) : Math.max(current - STEP_PER_TICK, target);
	}
}
