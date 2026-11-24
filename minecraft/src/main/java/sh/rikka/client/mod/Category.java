package sh.rikka.client.mod;

import java.util.Locale;
import net.minecraft.network.chat.Component;
import sh.rikka.client.RikkaClient;

public enum Category {
	HUD,
	VISUAL,
	MECHANIC,
	UTILITY;

	public Component displayName() {
		return Component.translatable(RikkaClient.MOD_ID + ".category." + name().toLowerCase(Locale.ROOT));
	}
}
