package sh.rikka.client.mods.item;

import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;

public final class Items2DMod extends Mod {
	private static volatile @Nullable Items2DMod active;

	public Items2DMod() {
		super("items_2d", Category.VISUAL, "layers");
	}

	static @Nullable Items2DMod active() {
		return active;
	}

	@Override
	protected void onEnable() {
		ItemPhysicsMod physics = ItemPhysicsMod.active();
		if (physics != null) {
			physics.setEnabled(false);
		}
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
	}
}
