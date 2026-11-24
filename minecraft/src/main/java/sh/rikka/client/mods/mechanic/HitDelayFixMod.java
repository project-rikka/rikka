package sh.rikka.client.mods.mechanic;

import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;

public final class HitDelayFixMod extends Mod {
	private static volatile boolean active;

	public HitDelayFixMod() {
		super("hit_delay_fix", Category.MECHANIC, "timer_off");
	}

	static boolean isActive() {
		return active;
	}

	@Override
	protected void onEnable() {
		active = true;
	}

	@Override
	protected void onDisable() {
		active = false;
	}
}
