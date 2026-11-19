package sh.rikka.client.mod;

import os.rikka.View;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.PositionSetting;
import sh.rikka.client.setting.ScreenPosition;

public abstract class HudMod extends Mod {
	private static final double MIN_SCALE = 0.5;
	private static final double MAX_SCALE = 3.0;
	private static final double SCALE_STEP = 0.05;

	private final PositionSetting position;
	private final DoubleSetting scale;

	@SuppressWarnings("this-escape")
	protected HudMod(String id, ScreenPosition defaultPosition) {
		super(id, Category.HUD);
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

	protected abstract void view(View view);

	public final void place(View view, float screenWidth, float screenHeight) {
		float scale = this.scale.getFloat();
		float width = width();
		float height = height();
		ScreenPosition placement = this.position.get();
		view.column(() -> view(view))
			.position(placement.left(screenWidth, width * scale), placement.top(screenHeight, height * scale))
			.width(width)
			.height(height)
			.scale(scale);
	}
}
