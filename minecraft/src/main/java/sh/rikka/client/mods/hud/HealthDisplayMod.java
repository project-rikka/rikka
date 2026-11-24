package sh.rikka.client.mods.hud;

import java.util.Locale;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.ScreenPosition;

public final class HealthDisplayMod extends SimpleHudMod {
	public HealthDisplayMod(ScreenPosition defaultPosition) {
		super("health_display", defaultPosition, "favorite");
	}

	@Override
	protected String label() {
		LocalPlayer player = Minecraft.getInstance().player;
		if (player == null) {
			return UNAVAILABLE;
		}
		String health = String.format(Locale.ROOT, "%.1f", player.getHealth());
		float absorption = player.getAbsorptionAmount();
		if (absorption <= 0.0F) {
			return translate("format", health);
		}
		return translate("format_absorption", health, String.format(Locale.ROOT, "%.1f", absorption));
	}
}
