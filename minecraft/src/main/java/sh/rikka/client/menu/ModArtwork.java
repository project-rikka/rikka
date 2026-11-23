package sh.rikka.client.menu;

import os.rikka.Align;
import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.Justification;
import os.rikka.View;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.ui.style.Colors;

record ModArtwork(float width, float height, float iconSize, float radius) {
	private static final float ENABLED_TINT = 0.18F;

	void view(View view, Mod mod) {
		boolean enabled = mod.isEnabled();
		Color accent = view.paletteColor(ColorRole.ACCENT);
		Color variant = view.paletteColor(ColorRole.SURFACE_VARIANT);
		view.box(() -> view.row(() -> view.icon(mod.icon())
					.iconSize(this.iconSize)
					.tint(enabled ? accent : view.paletteColor(ColorRole.TEXT_MUTED)))
				.width(this.width)
				.height(this.height)
				.justify(Justification.CENTER)
				.alignItems(Align.CENTER))
			.background(enabled ? Colors.mix(variant, accent, ENABLED_TINT) : variant)
			.cornerRadius(this.radius);
	}
}
