package sh.rikka.client.hud.style;

import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.View;
import sh.rikka.client.ui.style.Colors;

public record Accent(Color start, Color end) {
	private static final float THEME_TURN = 0.12F;

	public static Accent of(HudStyle style, AccentPalette palette, View view) {
		return switch (palette) {
			case THEME -> {
				Color accent = view.paletteColor(ColorRole.ACCENT);
				yield new Accent(accent, Colors.turned(accent, THEME_TURN));
			}
			case CUSTOM -> new Accent(Colors.of(style.accentStart().argb()), Colors.of(style.accentEnd().argb()));
			default -> new Accent(Colors.of(palette.start()), Colors.of(palette.end()));
		};
	}
}
