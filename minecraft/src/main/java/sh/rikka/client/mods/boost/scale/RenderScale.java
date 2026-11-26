package sh.rikka.client.mods.boost.scale;

import sh.rikka.client.mods.boost.BoostFeature;

public final class RenderScale {
	static final double NATIVE = 1.0;

	private static volatile double scale = NATIVE;
	private static volatile float sharpness;

	private RenderScale() {
	}

	public static void setScale(double value) {
		scale = value;
	}

	public static void setSharpness(double value) {
		sharpness = (float) value;
	}

	static double factor() {
		return BoostFeature.RENDER_SCALE.isActive() ? scale : NATIVE;
	}

	static float sharpness() {
		return sharpness;
	}
}
