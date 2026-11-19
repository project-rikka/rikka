package sh.rikka.client.mods.hud;

import java.time.LocalTime;
import java.time.format.DateTimeFormatter;
import java.util.Locale;
import net.minecraft.client.Minecraft;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class ClockMod extends SimpleHudMod {
	private final EnumSetting<HourCycle> hourCycle = choice("hour_cycle", HourCycle.H24);
	private final BoolSetting seconds = bool("seconds", false);
	private @Nullable Formatter formatter;

	public ClockMod(ScreenPosition defaultPosition) {
		super("clock", defaultPosition, "schedule", 12);
	}

	@Override
	protected String label() {
		return LocalTime.now().format(formatter());
	}

	private DateTimeFormatter formatter() {
		HourCycle cycle = this.hourCycle.get();
		boolean withSeconds = this.seconds.get();
		String language = Minecraft.getInstance().getLanguageManager().getSelected();
		Formatter current = this.formatter;
		if (current == null || !current.matches(cycle, withSeconds, language)) {
			Locale locale = Locale.forLanguageTag(language.replace('_', '-'));
			current = new Formatter(cycle, withSeconds, language, DateTimeFormatter.ofPattern(cycle.pattern(withSeconds), locale));
			this.formatter = current;
		}
		return current.format();
	}

	private record Formatter(HourCycle cycle, boolean seconds, String language, DateTimeFormatter format) {
		boolean matches(HourCycle cycle, boolean seconds, String language) {
			return this.cycle == cycle && this.seconds == seconds && this.language.equals(language);
		}
	}
}
