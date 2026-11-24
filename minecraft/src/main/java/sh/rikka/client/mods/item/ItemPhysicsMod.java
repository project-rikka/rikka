package sh.rikka.client.mods.item;

import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.DoubleSetting;

public final class ItemPhysicsMod extends Mod {
	private static volatile @Nullable ItemPhysicsMod active;

	private final DoubleSetting speed = number("speed", 1.0, 0.25, 4.0).step(0.05);

	public ItemPhysicsMod() {
		super("item_physics", Category.VISUAL, "deployed_code");
	}

	static @Nullable ItemPhysicsMod active() {
		return active;
	}

	float speed() {
		return this.speed.getFloat();
	}

	@Override
	protected void onEnable() {
		Items2DMod flat = Items2DMod.active();
		if (flat != null) {
			flat.setEnabled(false);
		}
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
	}
}
