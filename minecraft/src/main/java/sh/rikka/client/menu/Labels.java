package sh.rikka.client.menu;

import net.minecraft.client.resources.language.I18n;
import sh.rikka.client.RikkaClient;

public final class Labels {
	private static final String PREFIX = RikkaClient.MOD_ID + ".menu.";

	private Labels() {
	}

	public static String of(String suffix, Object... arguments) {
		return I18n.get(PREFIX + suffix, arguments);
	}
}
