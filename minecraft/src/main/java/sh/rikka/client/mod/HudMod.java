package sh.rikka.client.mod;

import net.minecraft.client.Minecraft;
import os.rikka.View;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.PositionSetting;
import sh.rikka.client.setting.ScreenPosition;

public abstract class HudMod extends Mod {
	public static final float UNITS_PER_GUI_PIXEL = 13.0F / 9.0F;

	private static final double MIN_SCALE = 0.5;
	private static final double MAX_SCALE = 3.0;
	private static final double SCALE_STEP = 0.05;

	private final PositionSetting position;
	private final DoubleSetting scale;

	@SuppressWarnings("this-escape")
	protected HudMod(String id, ScreenPosition defaultPosition, String icon) {
		super(id, Category.HUD, icon);
		this.position = position("position", defaultPosition);
		this.scale = number("scale", 1.0, MIN_SCALE, MAX_SCALE).step(SCALE_STEP);
	}

	public final PositionSetting position() {
		return this.position;
	}

	public final DoubleSetting scale() {
		return this.scale;
	}

	public abstract float width();

	public abstract float height();

	protected abstract void view(View view, HudLook look);

	public boolean animating() {
		return false;
	}

	protected static boolean editing() {
		return Minecraft.getInstance().gui.screen() instanceof HudEditing;
	}

	public final HudBounds bounds(float screenWidth, float screenHeight) {
		float scale = this.scale.getFloat();
		float width = width() * scale;
		float height = height() * scale;
		ScreenPosition placement = this.position.get();
		return new HudBounds(placement.left(screenWidth, width), placement.top(screenHeight, height), width, height);
	}

	public final void place(View view, HudLook look, float screenWidth, float screenHeight) {
		HudBounds bounds = bounds(screenWidth, screenHeight);
		view.column(() -> view(view, look))
			.position(bounds.left(), bounds.top())
			.width(width())
			.height(height())
			.scale(this.scale.getFloat());
	}
}
