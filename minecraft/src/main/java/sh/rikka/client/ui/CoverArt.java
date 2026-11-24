package sh.rikka.client.ui;

import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.ColorRole;
import os.rikka.Element;
import os.rikka.Fit;
import os.rikka.Image;
import os.rikka.Justification;
import os.rikka.View;

public final class CoverArt {
	private static final float ICON_SHARE = 0.5F;
	private static final String PLACEHOLDER = "music_note";

	private CoverArt() {
	}

	public static Element view(View view, @Nullable Image cover, float size, float radius) {
		return view.box(() -> {
				if (cover != null) {
					view.imageView(cover).imageFit(Fit.COVER).fillWidth().fillHeight();
				} else {
					view.column(() -> view.icon(PLACEHOLDER)
							.iconSize(size * ICON_SHARE)
							.tint(view.paletteColor(ColorRole.TEXT_MUTED)))
						.fillWidth()
						.fillHeight()
						.alignItems(Align.CENTER)
						.justify(Justification.CENTER);
				}
			})
			.width(size)
			.height(size)
			.shrink(0.0F)
			.background(view.paletteColor(ColorRole.SURFACE_VARIANT))
			.cornerRadius(radius)
			.clipContent();
	}
}
