package sh.rikka.client.mods.hud;

import java.util.Locale;
import net.minecraft.util.Util;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.HitEvent;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.IntSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class ReachDisplayMod extends SimpleHudMod {
	private static final long SHOWN_MILLIS = 5000;
	private static final int MAX_DECIMALS = 3;

	private final IntSetting decimals = integer("decimals", 2, 0, MAX_DECIMALS);
	private @Nullable Hit lastHit;

	public ReachDisplayMod(ScreenPosition defaultPosition) {
		super("reach_display", defaultPosition, "social_distance", 14);
	}

	@Override
	protected void onDisable() {
		this.lastHit = null;
	}

	@EventHandler
	private void onHit(HitEvent event) {
		this.lastHit = new Hit(event.reach(), Util.getMillis());
	}

	@Override
	protected String label() {
		Hit hit = this.lastHit;
		if (hit == null || Util.getMillis() - hit.at() > SHOWN_MILLIS) {
			return translate("idle");
		}
		return translate("format", String.format(Locale.ROOT, "%." + this.decimals.get() + "f", hit.reach()));
	}

	private record Hit(double reach, long at) {
	}
}
