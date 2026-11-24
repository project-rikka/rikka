package sh.rikka.client.ui;

import com.mojang.blaze3d.platform.Window;
import net.minecraft.client.Minecraft;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.HudRenderEvent;
import sh.rikka.client.mod.HudFrame;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.mod.ModManager;
import sh.rikka.client.mod.OverlayHudMod;

public final class HudOverlays {
	private final ModManager mods;

	public HudOverlays(ModManager mods) {
		this.mods = mods;
	}

	@EventHandler
	private void onHudRender(HudRenderEvent event) {
		Window window = Minecraft.getInstance().getWindow();
		int guiScale = window.getGuiScale();
		float physicalPerUnit = guiScale / HudMod.UNITS_PER_GUI_PIXEL;
		HudFrame frame = new HudFrame(
			event.graphics(),
			event.partialTick(),
			window.getWidth() / physicalPerUnit,
			window.getHeight() / physicalPerUnit,
			guiScale
		);
		for (Mod mod : this.mods.all()) {
			if (mod instanceof OverlayHudMod hud && hud.isEnabled()) {
				overlay(hud, frame);
			}
		}
	}

	private static void overlay(OverlayHudMod hud, HudFrame frame) {
		try {
			hud.overlay(frame);
		} catch (RuntimeException failure) {
			RikkaClient.LOGGER.error("{} failed to draw its overlay and is disabled", hud.id(), failure);
			hud.setEnabled(false);
		}
	}
}
