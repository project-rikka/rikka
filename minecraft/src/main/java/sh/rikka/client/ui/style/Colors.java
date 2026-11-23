package sh.rikka.client.ui.style;

import net.minecraft.util.ARGB;
import os.rikka.Color;

public final class Colors {
	private Colors() {
	}

	public static Color of(int argb) {
		return new Color(ARGB.redFloat(argb), ARGB.greenFloat(argb), ARGB.blueFloat(argb), ARGB.alphaFloat(argb));
	}

	public static Color withAlpha(Color color, float alpha) {
		return new Color(color.r(), color.g(), color.b(), alpha);
	}

	public static Color mix(Color from, Color to, float amount) {
		return new Color(
			from.r() + (to.r() - from.r()) * amount,
			from.g() + (to.g() - from.g()) * amount,
			from.b() + (to.b() - from.b()) * amount,
			from.a() + (to.a() - from.a()) * amount
		);
	}
}
