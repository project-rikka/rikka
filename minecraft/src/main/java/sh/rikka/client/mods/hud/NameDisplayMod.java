package sh.rikka.client.mods.hud;

import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.ScreenPosition;

public final class NameDisplayMod extends SimpleHudMod {
	public NameDisplayMod(ScreenPosition defaultPosition) {
		super("name_display", defaultPosition, "person");
	}

	@Override
	protected String label() {
		LocalPlayer player = Minecraft.getInstance().player;
		return player == null ? UNAVAILABLE : player.getName().getString();
	}
}
