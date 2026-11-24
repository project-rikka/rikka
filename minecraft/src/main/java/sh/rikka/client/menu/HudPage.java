package sh.rikka.client.menu;

import java.util.Locale;
import java.util.Optional;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.Element;
import os.rikka.FontWeight;
import os.rikka.Gradient;
import os.rikka.Justification;
import os.rikka.Rikka;
import os.rikka.TextRole;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.hud.style.Accent;
import sh.rikka.client.hud.style.AccentPalette;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.hud.style.HudStyle;
import sh.rikka.client.hud.style.HudTemplate;
import sh.rikka.client.menu.settings.SettingRows;
import sh.rikka.client.ui.style.Colors;

final class HudPage implements AutoCloseable {
	private static final float PREVIEW_HEIGHT = 132.0F;
	private static final float CARD_WIDTH = 176.0F;
	private static final float CARD_PREVIEW_HEIGHT = 72.0F;
	private static final float PILL_HEIGHT = 25.0F;
	private static final float PILL_PADDING = 7.0F;
	private static final float PILL_GAP = 6.0F;
	private static final float PILL_ICON = 15.0F;
	private static final float PILL_TEXT = 13.0F;
	private static final float SWATCH = 34.0F;
	private static final float SELECTED_BORDER = 2.0F;
	private static final float RESTING_BORDER = 1.0F;
	private static final float RESTING_BORDER_ALPHA = 0.35F;
	private static final float BACKDROP_ANGLE = 160.0F;
	private static final Color DUSK = new Color(0.16F, 0.23F, 0.42F, 1.0F);
	private static final Color FIELD = new Color(0.36F, 0.55F, 0.32F, 1.0F);
	private static final Color HORIZON = new Color(0.91F, 0.62F, 0.45F, 1.0F);
	private static final float HORIZON_OFFSET = 0.55F;
	private static final float SWATCH_ANGLE = 135.0F;
	private static final float SWATCH_ICON = 16.0F;
	private static final Color SWATCH_MARK = new Color(1.0F, 1.0F, 1.0F, 1.0F);

	private final SettingRows rows;
	private @Nullable Gradient backdrop;

	HudPage(SettingRows rows) {
		this.rows = rows;
	}

	static boolean isAnimated() {
		HudStyle style = RikkaClient.preferences().hud();
		return style.isAnimated() || style.isMoving();
	}

	void view(View view) {
		HudStyle style = RikkaClient.preferences().hud();
		Gradient backdrop = backdrop();
		Heading.view(view, Labels.of("hud.preview"));
		try (HudLook look = HudLook.of(style, view)) {
			stage(view, backdrop, PREVIEW_HEIGHT, () -> {
				pill(view, look, "speed", Labels.of("hud.sample.fps"));
				pill(view, look, "wifi", Labels.of("hud.sample.ping"));
				pill(view, look, "schedule", Labels.of("hud.sample.clock"));
			}).fillWidth();
		}
		Heading.view(view, Labels.of("hud.templates"));
		Optional<HudTemplate> worn = style.template();
		view.row(() -> {
				for (HudTemplate template : HudTemplate.values()) {
					templateCard(view, style, template, worn.filter(template::equals).isPresent(), backdrop);
				}
			})
			.wrapItems()
			.gap(Rikka.SPACE_NORMAL)
			.fillWidth();
		Heading.view(view, Labels.of("hud.accent"));
		accents(view, style);
		this.rows.grid(view, style.accentSettings());
		Heading.view(view, Labels.of("hud.panel"));
		this.rows.grid(view, style.panelSettings());
		Heading.view(view, Labels.of("hud.text"));
		this.rows.grid(view, style.textSettings());
	}

	private static Element stage(View view, Gradient backdrop, float height, Runnable content) {
		return view.box(() -> view.row(content)
				.fillWidth()
				.height(height)
				.gap(Rikka.SPACE_NORMAL)
				.justify(Justification.CENTER)
				.alignItems(Align.CENTER))
			.backgroundGradient(backdrop)
			.cornerRadius(Rikka.CORNER_ITEM)
			.clipContent();
	}

	private static void pill(View view, HudLook look, String icon, String text) {
		look.panel(view.box(() -> view.row(() -> {
					look.text(view.icon(icon).iconSize(PILL_ICON));
					look.text(view.text(text).textSize(PILL_TEXT).textNoWrap());
				})
				.fillHeight()
				.gap(PILL_GAP)
				.alignItems(Align.CENTER))
			.height(PILL_HEIGHT)
			.shrink(0.0F)
			.paddingEdges(0.0F, PILL_PADDING, 0.0F, PILL_PADDING));
	}

	private static void templateCard(View view, HudStyle style, HudTemplate template, boolean selected, Gradient backdrop) {
		Color accent = view.paletteColor(ColorRole.ACCENT);
		Color outline = view.paletteColor(ColorRole.OUTLINE);
		String name = Labels.of("hud.template." + template.name().toLowerCase(Locale.ROOT));
		view.box(() -> {
				try (HudLook look = HudLook.preview(style, template, view)) {
					stage(view, backdrop, CARD_PREVIEW_HEIGHT, () -> pill(view, look, "bolt", Labels.of("hud.sample.fps"))).fillWidth();
				}
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
			.key("hud-template:" + template.name())
			.onPress(context -> style.apply(template));
	}

	private static void accents(View view, HudStyle style) {
		Color selection = view.paletteColor(ColorRole.TEXT);
		Color outline = view.paletteColor(ColorRole.OUTLINE);
		AccentPalette chosen = style.accent().get();
		view.row(() -> {
				for (AccentPalette palette : AccentPalette.values()) {
					Accent accent = Accent.of(style, palette, view);
					boolean selected = palette == chosen;
					try (Gradient swatch = Gradient.fromLinear(SWATCH_ANGLE, accent.start(), accent.end())) {
						view.box(() -> {
								if (palette == AccentPalette.CUSTOM) {
									view.row(() -> view.icon("edit").iconSize(SWATCH_ICON).tint(SWATCH_MARK))
										.fillWidth()
										.fillHeight()
										.justify(Justification.CENTER)
										.alignItems(Align.CENTER);
								}
							})
							.width(SWATCH)
							.height(SWATCH)
							.cornerRadius(SWATCH / 2.0F)
							.backgroundGradient(swatch)
							.border(selected ? SELECTED_BORDER : RESTING_BORDER, selected ? selection : Colors.withAlpha(outline, RESTING_BORDER_ALPHA))
							.tooltip(style.accent().constantName(palette).getString())
							.key("hud-accent:" + palette.name())
							.onPress(context -> style.accent().set(palette));
					}
				}
			})
			.wrapItems()
			.gap(Rikka.SPACE_SNUG)
			.fillWidth();
	}

	private Gradient backdrop() {
		Gradient current = this.backdrop;
		if (current == null) {
			try (Gradient sky = Gradient.fromLinear(BACKDROP_ANGLE, DUSK, FIELD)) {
				current = sky.withStop(HORIZON_OFFSET, HORIZON);
			}
			this.backdrop = current;
		}
		return current;
	}

	@Override
	public void close() {
		Gradient current = this.backdrop;
		this.backdrop = null;
		if (current != null) {
			current.close();
		}
	}
}
