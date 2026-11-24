package sh.rikka.client.ui.style;

import net.minecraft.util.ARGB;
import net.minecraft.util.Mth;
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

	public static Color fade(Color color, float share) {
		return withAlpha(color, color.a() * Mth.clamp(share, 0.0F, 1.0F));
	}

	public static Color turned(Color color, float turns) {
		float max = Math.max(color.r(), Math.max(color.g(), color.b()));
		float min = Math.min(color.r(), Math.min(color.g(), color.b()));
		float chroma = max - min;
		float sector;
		if (chroma <= 0.0F) {
			sector = 0.0F;
		} else if (max == color.r()) {
			sector = Mth.positiveModulo((color.g() - color.b()) / chroma, 6.0F);
		} else if (max == color.g()) {
			sector = (color.b() - color.r()) / chroma + 2.0F;
		} else {
			sector = (color.r() - color.g()) / chroma + 4.0F;
		}
		float hue = Mth.positiveModulo(sector / 6.0F + turns, 1.0F);
		float saturation = max > 0.0F ? chroma / max : 0.0F;
		int rgb = Mth.hsvToArgb(hue, saturation, max, 0xFF);
		return new Color(ARGB.redFloat(rgb), ARGB.greenFloat(rgb), ARGB.blueFloat(rgb), color.a());
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
