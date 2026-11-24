package sh.rikka.client.menu;

import java.util.List;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.config.Preferences;
import sh.rikka.client.menu.settings.SettingRows;

final class PreferencesPage {
	private final SettingRows rows;

	PreferencesPage(SettingRows rows) {
		this.rows = rows;
	}

	void view(View view) {
		Preferences preferences = RikkaClient.preferences();
		Heading.view(view, Labels.of("preferences.keys"));
		this.rows.grid(view, List.of(preferences.menuKey(), preferences.hudEditorKey()));
	}
}
