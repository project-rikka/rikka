package sh.rikka.client.menu;

import sh.rikka.client.mod.Mod;

public sealed interface Page {
	record Mods() implements Page {
	}

	record ModSettings(Mod mod) implements Page {
	}

	record Music() implements Page {
	}

	record Profiles() implements Page {
	}

	record Themes() implements Page {
	}

	record Hud() implements Page {
	}

	record Preferences() implements Page {
	}
}
