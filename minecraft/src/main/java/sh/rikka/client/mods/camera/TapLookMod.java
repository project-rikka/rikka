package sh.rikka.client.mods.camera;

import com.mojang.blaze3d.platform.InputConstants;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.DefaultBind;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.EnumSetting;

public final class TapLookMod extends Mod {
	private static volatile @Nullable TapLookMod active;

	private final EnumSetting<Viewpoint> perspective = choice("perspective", Viewpoint.FRONT);

	public TapLookMod() {
		super("tap_look", Category.UTILITY, "touch_app", DefaultBind.hold(InputConstants.KEY_V));
	}

	static @Nullable TapLookMod active() {
		return active;
	}

	Viewpoint viewpoint() {
		return this.perspective.get();
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
