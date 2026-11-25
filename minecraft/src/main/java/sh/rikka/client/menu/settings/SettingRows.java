package sh.rikka.client.menu.settings;

import java.util.List;
import os.rikka.Align;
import os.rikka.Justification;
import os.rikka.Rikka;
import os.rikka.SizeClass;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.TransitionStyle;
import os.rikka.View;
import sh.rikka.client.menu.Labels;
import sh.rikka.client.setting.Setting;

public final class SettingRows {
	private static final int COLUMNS = 2;
	private static final float HEADING_BASIS = 140.0F;

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
		ControlPlacement placement = SettingControls.placement(setting);
		view.card(() -> {
				view.row(() -> {
						heading(view, setting);
						trailing(view, setting, placement);
					})
					.wrapItems()
					.justify(Justification.END)
					.alignItems(Align.CENTER)
					.gap(Rikka.SPACE_NORMAL)
					.fillWidth();
				if (placement == ControlPlacement.BELOW) {
					this.controls.control(view, setting);
				}
			})
			.gap(Rikka.SPACE_NORMAL)
			.key("setting:" + setting.translationKey())
			.transition(TransitionStyle.FADE_SCALE)
			.glide();
	}

	private static void heading(View view, Setting<?, ?> setting) {
		view.column(() -> {
				view.text(setting.name().getString()).textStyle(TextRole.SUBHEADING);
				view.text(setting.description().getString()).textStyle(TextRole.CAPTION).tone(TextTone.MUTED);
			})
			.gap(Rikka.SPACE_HAIR)
			.width(HEADING_BASIS)
			.grow(1.0F)
			.shrink(1.0F);
	}

	private void trailing(View view, Setting<?, ?> setting, ControlPlacement placement) {
		boolean modified = !setting.isDefault();
		if (!modified && placement == ControlPlacement.BELOW) {
			return;
		}
		view.row(() -> {
				if (modified) {
					view.iconButton("restart_alt")
						.controlSize(SizeClass.SMALL)
						.tooltip(Labels.of("reset"))
						.key("reset:" + setting.translationKey())
						.transition(TransitionStyle.POP)
						.onPress(context -> this.controls.reset(setting));
				}
				if (placement == ControlPlacement.TRAILING) {
					this.controls.control(view, setting);
				}
			})
			.gap(Rikka.SPACE_SNUG)
			.alignItems(Align.CENTER)
			.shrink(0.0F);
	}
}
