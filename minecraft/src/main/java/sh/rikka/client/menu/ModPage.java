package sh.rikka.client.menu;

import java.util.function.Consumer;
import os.rikka.Align;
import os.rikka.ButtonVariant;
import os.rikka.Justification;
import os.rikka.Rikka;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.View;
import sh.rikka.client.menu.settings.SettingControls;
import sh.rikka.client.menu.settings.SettingRows;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.Setting;

final class ModPage {
	private static final ModArtwork ARTWORK = new ModArtwork(64.0F, 64.0F, 34.0F, Rikka.CORNER_PANEL);

	private final SettingControls controls;
	private final SettingRows rows;
	private final Consumer<HudMod> editHud;

	ModPage(SettingControls controls, Consumer<HudMod> editHud) {
		this.controls = controls;
		this.rows = new SettingRows(controls);
		this.editHud = editHud;
	}

	void view(View view, Mod mod, boolean canEditHud) {
		header(view, mod, canEditHud);
		if (!mod.settings().all().isEmpty()) {
			Heading.view(view, Labels.of("mod.settings"));
			this.rows.grid(view, mod.settings().all());
		}
		Heading.view(view, Labels.of("mod.controls"));
		this.rows.grid(view, mod.controls().all());
	}

	private void header(View view, Mod mod, boolean canEditHud) {
		view.card(() -> {
				view.row(() -> {
						ARTWORK.view(view, mod);
						view.column(() -> {
								view.text(mod.name().getString()).textStyle(TextRole.TITLE);
								view.text(mod.description().getString()).tone(TextTone.MUTED);
							})
							.gap(Rikka.SPACE_TIGHT)
							.grow(1.0F)
							.shrink(1.0F);
						view.toggle(mod.isEnabled()).onToggle((context, on) -> mod.setEnabled(on));
					})
					.gap(Rikka.SPACE_ROOMY)
					.alignItems(Align.CENTER)
					.fillWidth();
				view.row(() -> {
						if (mod instanceof HudMod hud) {
							view.button(Labels.of("mod.edit_position"))
								.buttonStyle(ButtonVariant.SECONDARY)
								.enabled(canEditHud)
								.onPress(context -> this.editHud.accept(hud));
						}
						view.button(Labels.of("mod.reset"))
							.buttonStyle(ButtonVariant.GHOST)
							.onPress(context -> context.confirm(
								Labels.of("mod.reset.title", mod.name().getString()),
								Labels.of("mod.reset.message"),
								Labels.of("mod.reset.accept"),
								(answered, confirmed) -> {
									if (confirmed) {
										resetAll(mod);
									}
								}
							));
					})
					.gap(Rikka.SPACE_SNUG)
					.justify(Justification.END)
					.fillWidth();
			})
			.gap(Rikka.SPACE_ROOMY);
	}

	private void resetAll(Mod mod) {
		for (Setting<?, ?> setting : mod.settings().all()) {
			this.controls.reset(setting);
		}
		for (Setting<?, ?> setting : mod.controls().all()) {
			this.controls.reset(setting);
		}
	}
}
