package sh.rikka.client.menu.settings;

import java.util.List;
import os.rikka.Align;
import os.rikka.Rikka;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.TransitionStyle;
import os.rikka.View;
import sh.rikka.client.menu.Labels;
import sh.rikka.client.setting.Setting;

public final class SettingRows {
	private static final int COLUMNS = 2;
	private static final float HEADING_MIN_WIDTH = 140.0F;

	private final SettingControls controls;

	public SettingRows(SettingControls controls) {
		this.controls = controls;
	}

	public void grid(View view, List<? extends Setting<?, ?>> settings) {
		view.grid(COLUMNS, () -> {
				for (Setting<?, ?> setting : settings) {
					if (setting.isVisible()) {
						row(view, setting);
					}
				}
			})
			.gap(Rikka.SPACE_NORMAL)
			.fillWidth();
	}

	private void row(View view, Setting<?, ?> setting) {
		boolean wide = SettingControls.isWide(setting);
		view.card(() -> {
				if (wide) {
					view.row(() -> heading(view, setting)).alignItems(Align.START).gap(Rikka.SPACE_SNUG).fillWidth();
					this.controls.control(view, setting);
				} else {
					view.row(() -> {
							heading(view, setting);
							this.controls.control(view, setting);
						})
						.alignItems(Align.CENTER)
						.gap(Rikka.SPACE_NORMAL)
						.fillWidth();
				}
			})
			.gap(Rikka.SPACE_NORMAL)
			.key("setting:" + setting.translationKey())
			.transition(TransitionStyle.FADE_SCALE)
			.glide();
	}

	private void heading(View view, Setting<?, ?> setting) {
		view.column(() -> {
				view.text(setting.name().getString()).textStyle(TextRole.SUBHEADING);
				view.text(setting.description().getString()).textStyle(TextRole.CAPTION).tone(TextTone.MUTED);
			})
			.gap(Rikka.SPACE_HAIR)
			.grow(1.0F)
			.shrink(1.0F)
			.minWidth(HEADING_MIN_WIDTH);
		if (!setting.isDefault()) {
			view.iconButton("restart_alt")
				.tooltip(Labels.of("reset"))
				.key("reset:" + setting.translationKey())
				.transition(TransitionStyle.POP)
				.onPress(context -> this.controls.reset(setting));
		}
	}
}
