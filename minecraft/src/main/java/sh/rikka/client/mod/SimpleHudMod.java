package sh.rikka.client.mod;

import net.minecraft.client.resources.language.I18n;
import net.minecraft.util.ARGB;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.Color;
import os.rikka.Element;
import os.rikka.Justification;
import os.rikka.View;
import sh.rikka.client.setting.Anchor;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.ScreenPosition;

public abstract class SimpleHudMod extends HudMod {
	public static final String UNAVAILABLE = "—";
	public static final float HEIGHT = 24.0F;

	private static final float PADDING = 8.0F;
	private static final float GAP = 6.0F;
	private static final float ICON_SIZE = 16.0F;
	private static final float TEXT_SIZE = 13.0F;
	private static final float CHARACTER_WIDTH = 8.0F;
	private static final float CORNER_RADIUS = 8.0F;
	private static final int DEFAULT_COLOR = 0xFFFFFFFF;

	private final String icon;
	private final int maxCharacters;
	private final BoolSetting showIcon;
	private final BoolSetting showBackground;
	private final BoolSetting customColor;
	private final ColorSetting color;

	@SuppressWarnings("this-escape")
	protected SimpleHudMod(String id, ScreenPosition defaultPosition, String icon, int maxCharacters) {
		super(id, defaultPosition);
		this.icon = icon;
		this.maxCharacters = maxCharacters;
		this.showIcon = bool("icon", true);
		this.showBackground = bool("background", true);
		this.customColor = bool("custom_color", false);
		this.color = color("color", DEFAULT_COLOR).visibleWhen(this.customColor::get);
	}

	protected abstract String label();

	protected String icon() {
		return this.icon;
	}

	protected final String translate(String suffix, Object... arguments) {
		return I18n.get(translationKey() + "." + suffix, arguments);
	}

	@Override
	public final float width() {
		float iconWidth = this.showIcon.get() ? ICON_SIZE + GAP : 0.0F;
		return PADDING * 2.0F + iconWidth + this.maxCharacters * CHARACTER_WIDTH;
	}

	@Override
	public final float height() {
		return HEIGHT;
	}

	@Override
	protected final void view(View view) {
		String text = label();
		@Nullable String icon = this.showIcon.get() ? icon() : null;
		@Nullable Color tint = this.customColor.get() ? rikkaColor(this.color.argb()) : null;
		view.row(() -> panel(view, text, icon, tint))
			.fillWidth()
			.fillHeight()
			.justify(justification(position().get().anchor()))
			.alignItems(Align.CENTER);
	}

	private void panel(View view, String text, @Nullable String icon, @Nullable Color tint) {
		Element panel = view.box(() -> view.row(() -> content(view, text, icon, tint))
				.fillHeight()
				.gap(GAP)
				.alignItems(Align.CENTER))
			.height(HEIGHT)
			.paddingEdges(0.0F, PADDING, 0.0F, PADDING)
			.cornerRadius(CORNER_RADIUS);
		if (this.showBackground.get()) {
			panel.glass();
		}
	}

	private static void content(View view, String text, @Nullable String icon, @Nullable Color tint) {
		if (icon != null) {
			Element glyph = view.icon(icon).iconSize(ICON_SIZE).iconFilled();
			if (tint != null) {
				glyph.tint(tint);
			}
		}
		Element label = view.text(text).textSize(TEXT_SIZE).textNoWrap();
		if (tint != null) {
			label.tint(tint);
		}
	}

	private static Justification justification(Anchor anchor) {
		return switch (anchor) {
			case TOP_LEFT, CENTER_LEFT, BOTTOM_LEFT -> Justification.START;
			case TOP_CENTER, CENTER, BOTTOM_CENTER -> Justification.CENTER;
			case TOP_RIGHT, CENTER_RIGHT, BOTTOM_RIGHT -> Justification.END;
		};
	}

	private static Color rikkaColor(int argb) {
		return new Color(ARGB.redFloat(argb), ARGB.greenFloat(argb), ARGB.blueFloat(argb), ARGB.alphaFloat(argb));
	}
}
