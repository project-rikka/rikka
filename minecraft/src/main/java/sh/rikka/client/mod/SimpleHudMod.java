package sh.rikka.client.mod;

import net.minecraft.client.resources.language.I18n;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.Color;
import os.rikka.Element;
import os.rikka.Rikka;
import os.rikka.View;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.ScreenPosition;
import sh.rikka.client.ui.style.Colors;

public abstract class SimpleHudMod extends HudMod {
	public static final String UNAVAILABLE = "—";
	public static final float HEIGHT = 25.0F;

	private static final float PADDING = 7.0F;
	private static final float GAP = 6.0F;
	private static final float ICON_SIZE = 15.0F;
	private static final float TEXT_SIZE = 13.0F;
	private static final float CORNER_RADIUS = 9.0F;
	private static final int DEFAULT_COLOR = 0xFFFFFFFF;
	private static final Color BACKDROP = new Color(0.0F, 0.0F, 0.0F, 0.4F);

	private final BoolSetting showIcon;
	private final BoolSetting showBackground;
	private final BoolSetting customColor;
	private final ColorSetting color;

	@SuppressWarnings("this-escape")
	protected SimpleHudMod(String id, ScreenPosition defaultPosition, String icon) {
		super(id, defaultPosition, icon);
		this.showIcon = bool("icon", true);
		this.showBackground = bool("background", true);
		this.customColor = bool("custom_color", false);
		this.color = color("color", DEFAULT_COLOR).visibleWhen(this.customColor::get);
	}

	protected abstract String label();

	protected String shownIcon() {
		return icon();
	}

	protected final String translate(String suffix, Object... arguments) {
		return I18n.get(translationKey() + "." + suffix, arguments);
	}

	@Override
	public final float width() {
		float iconWidth = this.showIcon.get() ? ICON_SIZE + GAP : 0.0F;
		return PADDING * 2.0F + iconWidth + (float) Math.ceil(Rikka.textWidth(label(), TEXT_SIZE));
	}

	@Override
	public final float height() {
		return HEIGHT;
	}

	@Override
	protected final void view(View view) {
		String text = label();
		@Nullable String icon = this.showIcon.get() ? shownIcon() : null;
		Color tint = Colors.of(this.customColor.get() ? this.color.argb() : DEFAULT_COLOR);
		view.row(() -> panel(view, text, icon, tint))
			.fillWidth()
			.fillHeight()
			.alignItems(Align.CENTER);
	}

	private void panel(View view, String text, @Nullable String icon, Color tint) {
		Element panel = view.box(() -> view.row(() -> content(view, text, icon, tint))
				.fillHeight()
				.gap(GAP)
				.alignItems(Align.CENTER))
			.height(HEIGHT)
			.shrink(0.0F)
			.paddingEdges(0.0F, PADDING, 0.0F, PADDING)
			.cornerRadius(CORNER_RADIUS);
		if (this.showBackground.get()) {
			panel.glass().background(BACKDROP);
		}
	}

	private static void content(View view, String text, @Nullable String icon, Color tint) {
		if (icon != null) {
			view.icon(icon).iconSize(ICON_SIZE).tint(tint);
		}
		view.text(text).textSize(TEXT_SIZE).textNoWrap().tint(tint);
	}
}
