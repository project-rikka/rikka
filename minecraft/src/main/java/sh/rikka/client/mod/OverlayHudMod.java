package sh.rikka.client.mod;

import org.joml.Matrix3x2fStack;
import sh.rikka.client.setting.ScreenPosition;

public abstract class OverlayHudMod extends HudMod {
	protected OverlayHudMod(String id, ScreenPosition defaultPosition, String icon) {
		super(id, defaultPosition, icon);
	}

	protected abstract void extract(HudCanvas canvas);

	public final void overlay(HudFrame frame) {
		HudBounds bounds = bounds(frame.screenWidth(), frame.screenHeight());
		float guiPerUnit = 1.0F / UNITS_PER_GUI_PIXEL;
		float scale = scale().getFloat();
		Matrix3x2fStack pose = frame.graphics().pose();
		pose.pushMatrix();
		try {
			pose.translate(bounds.left() * guiPerUnit, bounds.top() * guiPerUnit);
			pose.scale(scale * guiPerUnit);
			extract(new HudCanvas(frame, 1.0F / (frame.guiScale() * guiPerUnit * scale)));
		} finally {
			pose.popMatrix();
		}
	}
}
