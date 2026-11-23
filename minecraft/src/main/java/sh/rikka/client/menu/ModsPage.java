package sh.rikka.client.menu;

import java.util.List;
import java.util.Optional;
import os.rikka.Align;
import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.FontWeight;
import os.rikka.Rikka;
import os.rikka.SizeClass;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.TransitionStyle;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.ui.style.Colors;

final class ModsPage {
	private static final float SEGMENT_WIDTH = 104.0F;
	private static final float CARD_WIDTH = 196.0F;
	private static final ModArtwork ARTWORK = new ModArtwork(CARD_WIDTH - Rikka.SPACE_SNUG * 2.0F, 92.0F, 40.0F, Rikka.CORNER_ITEM);
	private static final float BORDER_WIDTH = 1.5F;

	private ModsPage() {
	}

	static void view(View view, MenuState state) {
		Category[] categories = Category.values();
		List<Mod> shown = state.shown(RikkaClient.mods().all());
		view.segmented(state.category().map(category -> category.ordinal() + 1).orElse(0), () -> {
				view.segment(Labels.of("category.all"));
				for (Category category : categories) {
					view.segment(category.displayName().getString());
				}
			})
			.width(SEGMENT_WIDTH * (categories.length + 1))
			.onSelect((context, index) -> state.filter(index == 0 ? Optional.empty() : Optional.of(categories[index - 1])));
		if (shown.isEmpty()) {
			view.statusPage("search_off", Labels.of("mods.empty"), () -> {
				})
				.fillWidth()
				.key("mods:empty")
				.transition(TransitionStyle.FADE);
			return;
		}
		view.row(() -> {
				for (Mod mod : shown) {
					card(view, state, mod);
				}
			})
			.wrapItems()
			.gap(Rikka.SPACE_NORMAL)
			.fillWidth();
	}

	private static void card(View view, MenuState state, Mod mod) {
		boolean enabled = mod.isEnabled();
		Color accent = view.paletteColor(ColorRole.ACCENT);
		Color outline = view.paletteColor(ColorRole.OUTLINE);
		view.box(() -> {
				ARTWORK.view(view, mod);
				view.row(() -> {
						view.column(() -> {
								view.text(mod.name().getString()).textWeight(FontWeight.SEMIBOLD).textNoWrap();
								view.text(mod.category().displayName().getString()).textStyle(TextRole.CAPTION).tone(TextTone.MUTED);
							})
							.gap(Rikka.SPACE_HAIR)
							.grow(1.0F)
							.shrink(1.0F);
						view.toggle(enabled).controlSize(SizeClass.SMALL).onToggle((context, on) -> mod.setEnabled(on));
					})
					.gap(Rikka.SPACE_SNUG)
					.alignItems(Align.CENTER)
					.fillWidth();
			})
			.gap(Rikka.SPACE_SNUG)
			.padding(Rikka.SPACE_SNUG)
			.width(CARD_WIDTH)
			.surface()
			.cornerRadius(Rikka.CORNER_PANEL)
			.border(BORDER_WIDTH, enabled ? accent : Colors.withAlpha(outline, 0.0F))
			.key("mod:" + mod.id())
			.transition(TransitionStyle.FADE_SCALE)
			.glide()
			.onPress(context -> state.open(new Page.ModSettings(mod)));
	}
}
