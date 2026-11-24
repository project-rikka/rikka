package sh.rikka.client.menu;

import os.rikka.View;
import sh.rikka.client.menu.settings.SettingControls;
import sh.rikka.client.menu.settings.SettingRows;
import sh.rikka.client.mod.Mod;

final class ModPage {
	private final SettingRows rows;

	ModPage(SettingControls controls) {
		this.rows = new SettingRows(controls);
	}

	void view(View view, Mod mod) {
		if (!mod.settings().all().isEmpty()) {
			Heading.view(view, Labels.of("mod.settings"));
			this.rows.grid(view, mod.settings().all());
		}
		Heading.view(view, Labels.of("mod.controls"));
		this.rows.grid(view, mod.controls().all());
	}
}
