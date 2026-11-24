package sh.rikka.client.mods.visual;

import java.time.LocalTime;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetting;

public final class TimeChangerMod extends Mod {
	private static final long TICKS_PER_DAY = 24000L;
	private static final long TICKS_PER_HOUR = 1000L;
	private static final double SUNRISE_HOUR = 6.0;
	private static final double HOURS_PER_DAY = 24.0;
	private static final double SECONDS_PER_HOUR = 3600.0;
	private static volatile @Nullable TimeChangerMod active;

	private final EnumSetting<TimeMode> mode = choice("mode", TimeMode.FIXED);
	private final DoubleSetting time = number("time", 12.0, 0.0, 24.0).step(0.25).visibleWhen(() -> this.mode.is(TimeMode.FIXED));

	public TimeChangerMod() {
		super("time_changer", Category.VISUAL, "routine");
	}

	static @Nullable TimeChangerMod active() {
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

	long clockTicks(long vanilla) {
		long day = Math.floorDiv(vanilla, TICKS_PER_DAY);
		return day * TICKS_PER_DAY + timeOfDay(hours());
	}

	private double hours() {
		return switch (this.mode.get()) {
			case FIXED -> this.time.get();
			case REAL_TIME -> LocalTime.now().toSecondOfDay() / SECONDS_PER_HOUR;
		};
	}

	private static long timeOfDay(double hours) {
		double sinceSunrise = ((hours - SUNRISE_HOUR) % HOURS_PER_DAY + HOURS_PER_DAY) % HOURS_PER_DAY;
		return Math.round(sinceSunrise * TICKS_PER_HOUR) % TICKS_PER_DAY;
	}
}
