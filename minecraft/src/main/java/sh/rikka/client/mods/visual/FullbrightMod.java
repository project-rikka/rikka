package sh.rikka.client.mods.visual;

import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.DoubleSetting;

public final class FullbrightMod extends Mod {
	private static final double MIN_GAMMA = 1.0;
	private static final double MAX_GAMMA = 15.0;
	private static volatile @Nullable FullbrightMod active;

	private final DoubleSetting gamma = number("gamma", MAX_GAMMA, MIN_GAMMA, MAX_GAMMA).step(1.0);

	public FullbrightMod() {
		super("fullbright", Category.VISUAL, "lightbulb");
	}

	static @Nullable FullbrightMod active() {
		return active;
	}

	float brightness(float vanilla) {
		return Math.max(vanilla, this.gamma.getFloat());
	}

	@Override
	protected void onEnable() {
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
	}
}
