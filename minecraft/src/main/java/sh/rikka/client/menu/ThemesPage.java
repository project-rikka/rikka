package sh.rikka.client.menu;

import java.util.Locale;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.FontWeight;
import os.rikka.Justification;
import os.rikka.Rikka;
import os.rikka.TextRole;
import os.rikka.ThemeCatalog;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.config.Background;
import sh.rikka.client.config.Preferences;
import sh.rikka.client.ui.style.Colors;

final class ThemesPage implements AutoCloseable {
	private static final float SEGMENT_WIDTH = 104.0F;
	private static final float CARD_WIDTH = 176.0F;
	private static final float PREVIEW_HEIGHT = 92.0F;
	private static final float WINDOW_WIDTH = 112.0F;
	private static final float LINE_HEIGHT = 6.0F;
	private static final float TITLE_LINE_WIDTH = 64.0F;
	private static final float BODY_LINE_WIDTH = 44.0F;
	private static final float PILL_WIDTH = 34.0F;
	private static final float PILL_HEIGHT = 12.0F;
	private static final float DESKTOP_ICON_SIZE = 36.0F;
	private static final float SELECTED_BORDER = 2.0F;
	private static final float RESTING_BORDER = 1.0F;
	private static final float RESTING_BORDER_ALPHA = 0.35F;

	private @Nullable ThemeCatalog catalog;

	void view(View view) {
		Preferences preferences = RikkaClient.preferences();
		Background[] backgrounds = Background.values();
		Heading.view(view, Labels.of("themes.background"));
		view.segmented(preferences.background().get().ordinal(), () -> {
				for (Background background : backgrounds) {
					view.segment(Labels.of("themes.background." + background.name().toLowerCase(Locale.ROOT)));
				}
			})
			.width(SEGMENT_WIDTH * backgrounds.length)
			.onSelect((context, index) -> preferences.background().set(backgrounds[index]));
		Heading.view(view, Labels.of("themes.theme"));
		ThemeCatalog themes = catalog();
		String worn = preferences.theme().get();
		view.row(() -> {
				desktopCard(view, worn.isEmpty(), preferences);
				for (int index = 0; index < themes.getCount(); index++) {
					themeCard(view, themes, index, worn, preferences);
				}
			})
			.wrapItems()
			.gap(Rikka.SPACE_NORMAL)
			.fillWidth();
	}

	private void desktopCard(View view, boolean selected, Preferences preferences) {
		Color variant = view.paletteColor(ColorRole.SURFACE_VARIANT);
		card(view, selected, Labels.of("themes.desktop"), "theme:desktop", () -> preferences.theme().set(""), () ->
			view.box(() -> view.row(() -> view.icon("desktop_windows")
						.iconSize(DESKTOP_ICON_SIZE)
						.tint(view.paletteColor(ColorRole.TEXT_MUTED)))
					.fillWidth()
					.height(PREVIEW_HEIGHT)
					.justify(Justification.CENTER)
					.alignItems(Align.CENTER))
				.background(variant)
				.cornerRadius(Rikka.CORNER_ITEM)
				.fillWidth()
		);
	}

	private void themeCard(View view, ThemeCatalog themes, int index, String worn, Preferences preferences) {
		String slug = themes.slug(index);
		card(view, slug.equals(worn), themes.name(index), "theme:" + slug, () -> preferences.theme().set(slug), () ->
			view.box(() -> view.row(() -> miniature(view, themes, index))
					.fillWidth()
					.height(PREVIEW_HEIGHT)
					.justify(Justification.CENTER)
					.alignItems(Align.CENTER))
				.background(themes.color(index, ColorRole.BACKGROUND))
				.border(RESTING_BORDER, Colors.withAlpha(themes.color(index, ColorRole.OUTLINE), RESTING_BORDER_ALPHA))
				.cornerRadius(Rikka.CORNER_ITEM)
				.fillWidth()
		);
	}

	private static void miniature(View view, ThemeCatalog themes, int index) {
		view.box(() -> {
				line(view, themes.color(index, ColorRole.TEXT), TITLE_LINE_WIDTH);
				line(view, themes.color(index, ColorRole.TEXT_MUTED), BODY_LINE_WIDTH);
				view.row(() -> {
						view.box(() -> {
							})
							.width(PILL_WIDTH)
							.height(PILL_HEIGHT)
							.background(themes.color(index, ColorRole.ACCENT))
							.cornerRadius(PILL_HEIGHT / 2.0F);
						view.box(() -> {
							})
							.width(PILL_WIDTH)
							.height(PILL_HEIGHT)
							.background(themes.color(index, ColorRole.SURFACE_VARIANT))
							.cornerRadius(PILL_HEIGHT / 2.0F);
					})
					.gap(Rikka.SPACE_TIGHT);
			})
			.gap(Rikka.SPACE_TIGHT)
			.padding(Rikka.SPACE_SNUG)
			.width(WINDOW_WIDTH)
			.background(themes.color(index, ColorRole.SURFACE))
			.cornerRadius(Rikka.CORNER_ITEM);
	}

	private static void line(View view, Color color, float width) {
		view.box(() -> {
			})
			.width(width)
			.height(LINE_HEIGHT)
			.background(color)
			.cornerRadius(LINE_HEIGHT / 2.0F);
	}

	private static void card(View view, boolean selected, String name, String key, Runnable choose, Runnable preview) {
		Color accent = view.paletteColor(ColorRole.ACCENT);
		Color outline = view.paletteColor(ColorRole.OUTLINE);
		view.box(() -> {
				preview.run();
				view.row(() -> {
						view.text(name).textStyle(TextRole.LABEL).textWeight(FontWeight.MEDIUM).textNoWrap().grow(1.0F).shrink(1.0F);
						if (selected) {
							view.icon("check_circle").iconFilled().tint(accent);
						}
					})
					.gap(Rikka.SPACE_TIGHT)
					.alignItems(Align.CENTER)
					.fillWidth();
			})
			.gap(Rikka.SPACE_SNUG)
			.padding(Rikka.SPACE_SNUG)
			.width(CARD_WIDTH)
			.surface()
			.cornerRadius(Rikka.CORNER_PANEL)
			.border(selected ? SELECTED_BORDER : RESTING_BORDER, selected ? accent : Colors.withAlpha(outline, RESTING_BORDER_ALPHA))
			.key(key)
			.onPress(context -> choose.run());
	}

	private ThemeCatalog catalog() {
		ThemeCatalog current = this.catalog;
		if (current == null) {
			current = new ThemeCatalog();
			this.catalog = current;
		}
		return current;
	}

	@Override
	public void close() {
		ThemeCatalog current = this.catalog;
		this.catalog = null;
		if (current != null) {
			current.close();
		}
	}
}
