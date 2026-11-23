package sh.rikka.client.mods.visual;

import net.minecraft.world.entity.HumanoidArm;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.EnumSetting;

public final class ForceMainHandMod extends Mod {
	private static volatile @Nullable ForceMainHandMod active;

	private final EnumSetting<HumanoidArm> hand = choice("hand", HumanoidArm.RIGHT);

	public ForceMainHandMod() {
		super("force_main_hand", Category.VISUAL, "front_hand");
	}

	static @Nullable ForceMainHandMod active() {
		return active;
	}

	HumanoidArm arm() {
		return this.hand.get();
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
