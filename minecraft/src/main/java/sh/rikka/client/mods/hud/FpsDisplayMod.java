package sh.rikka.client.mods.hud;

import net.minecraft.client.Minecraft;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.setting.ScreenPosition;

public final class FpsDisplayMod extends SimpleHudMod {
	public FpsDisplayMod(ScreenPosition defaultPosition) {
		super("fps_display", defaultPosition, "monitor", 9);
	}

	@Override
	protected String label() {
		return translate("format", Minecraft.getInstance().getFps());
	}
}
