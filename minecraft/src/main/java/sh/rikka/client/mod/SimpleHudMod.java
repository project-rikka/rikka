package sh.rikka.client.mod;

import net.minecraft.client.resources.language.I18n;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.Element;
import os.rikka.Rikka;
import os.rikka.View;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ScreenPosition;

public abstract class SimpleHudMod extends HudMod {
	public static final String UNAVAILABLE = "—";
	public static final float HEIGHT = 25.0F;

	private static final float PADDING = 7.0F;
	private static final float GAP = 6.0F;
	private static final float ICON_SIZE = 15.0F;
	private static final float TEXT_SIZE = 13.0F;

	private final BoolSetting showIcon;
	private final BoolSetting showBackground;

	@SuppressWarnings("this-escape")
	protected SimpleHudMod(String id, ScreenPosition defaultPosition, String icon) {
		super(id, defaultPosition, icon);
		this.showIcon = bool("icon", true);
		this.showBackground = bool("background", true);
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
	protected final void view(View view, HudLook look) {
		String text = label();
		@Nullable String icon = this.showIcon.get() ? shownIcon() : null;
		view.row(() -> panel(view, look, text, icon))
			.fillWidth()
			.fillHeight()
			.alignItems(Align.CENTER);
	}

	private void panel(View view, HudLook look, String text, @Nullable String icon) {
		Element panel = view.box(() -> view.row(() -> content(view, look, text, icon))
				.fillHeight()
				.gap(GAP)
				.alignItems(Align.CENTER))
			.height(HEIGHT)
			.shrink(0.0F)
			.paddingEdges(0.0F, PADDING, 0.0F, PADDING);
		if (this.showBackground.get()) {
			look.panel(panel);
		}
	}

	private static void content(View view, HudLook look, String text, @Nullable String icon) {
		if (icon != null) {
			look.text(view.icon(icon).iconSize(ICON_SIZE));
		}
		look.text(view.text(text).textSize(TEXT_SIZE).textNoWrap());
	}
}
