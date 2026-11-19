package sh.rikka.client.mods.hud;

import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.world.timeline.Timelines;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.ScreenPosition;

public final class DayCounterMod extends SimpleHudMod {
	public DayCounterMod(ScreenPosition defaultPosition) {
		super("day_counter", defaultPosition, "today", 12);
	}

	@Override
	protected String label() {
		ClientLevel level = Minecraft.getInstance().level;
		if (level == null) {
			return UNAVAILABLE;
		}
		return level.registryAccess()
			.get(Timelines.OVERWORLD_DAY)
			.map(day -> translate("format", day.value().getPeriodCount(level.clockManager())))
			.orElse(UNAVAILABLE);
	}
}
