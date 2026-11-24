package sh.rikka.client.resource.legacy;

final class Colors {
	private Colors() {
	}

	public static int alpha(int argb) {
		return argb >>> 24;
	}

	public static int red(int argb) {
		return argb >> 16 & 0xFF;
	}

	public static int green(int argb) {
		return argb >> 8 & 0xFF;
	}

	public static int blue(int argb) {
		return argb & 0xFF;
	}

	public static int argb(int alpha, int red, int green, int blue) {
		return clamp(alpha) << 24 | clamp(red) << 16 | clamp(green) << 8 | clamp(blue);
	}

	public static int withAlpha(int argb, int alpha) {
		return clamp(alpha) << 24 | argb & 0xFFFFFF;
	}

	public static int distance(int a, int b) {
		int dr = red(a) - red(b);
		int dg = green(a) - green(b);
		int db = blue(a) - blue(b);
		int da = alpha(a) - alpha(b);
		return dr * dr + dg * dg + db * db + da * da;
	}

	public static int lerp(int a, int b, double t) {
		return argb(
			(int) Math.round(alpha(a) + (alpha(b) - alpha(a)) * t),
			(int) Math.round(red(a) + (red(b) - red(a)) * t),
			(int) Math.round(green(a) + (green(b) - green(a)) * t),
			(int) Math.round(blue(a) + (blue(b) - blue(a)) * t)
		);
	}

	private static int clamp(int channel) {
		return Math.max(0, Math.min(255, channel));
	}
}
