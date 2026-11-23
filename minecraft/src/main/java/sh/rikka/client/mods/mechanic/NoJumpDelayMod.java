package sh.rikka.client.mods.mechanic;

import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;

public final class NoJumpDelayMod extends Mod {
	private static volatile boolean active;

	public NoJumpDelayMod() {
		super("no_jump_delay", Category.MECHANIC, "keyboard_double_arrow_up");
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
