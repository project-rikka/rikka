package sh.rikka.client.mods.camera;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.Minecraft;
import net.minecraft.util.Mth;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.MouseScrollEvent;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.DefaultBind;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.DoubleSetting;

public final class ZoomMod extends Mod {
	private static final Zoom ZOOM = new Zoom();
	private static final double MIN_FACTOR = 1.5;
	private static final double MAX_FACTOR = 30.0;
	private static final double SCROLL_STEP = 0.12;
	private static final double INSTANT = 0.0;

	private final DoubleSetting factor = number("factor", 4.0, MIN_FACTOR, MAX_FACTOR).step(0.5);
	private final BoolSetting scroll = bool("scroll", true);
	private final BoolSetting scaleSensitivity = bool("scale_sensitivity", true);
	private final BoolSetting smooth = bool("smooth", true);
	private final DoubleSetting speed = number("speed", 14.0, 2.0, 30.0).step(1.0).visibleWhen(this.smooth::get);

	public ZoomMod() {
		super("zoom", Category.UTILITY, "zoom_in", DefaultBind.hold(InputConstants.KEY_C));
	}

	static float fov(float vanilla) {
		return ZOOM.fov(vanilla);
	}

	static double sensitivity() {
		return ZOOM.sensitivity();
	}

	@Override
	protected void onEnable() {
		aim(Math.log(this.factor.get()));
	}

	@Override
	protected void onDisable() {
		aim(0.0);
	}

	@EventHandler
	private void onScroll(MouseScrollEvent event) {
		if (!this.scroll.get() || event.vertical() == 0.0 || Minecraft.getInstance().gui.screen() != null) {
			return;
		}
		double exponent = ZOOM.target() + event.vertical() * SCROLL_STEP;
		aim(Mth.clamp(exponent, Math.log(MIN_FACTOR), Math.log(MAX_FACTOR)));
		event.cancel();
	}

	private void aim(double exponent) {
		ZOOM.aim(exponent, this.smooth.get() ? this.speed.get() : INSTANT, this.scaleSensitivity.get());
	}
}
