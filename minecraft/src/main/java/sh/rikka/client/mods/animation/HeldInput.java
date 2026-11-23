package sh.rikka.client.mods.animation;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;

final class HeldInput {
	private HeldInput() {
	}

	static boolean held(Minecraft minecraft, KeyMapping key) {
		return minecraft.player != null && key.isDown() && minecraft.gui.screen() == null && minecraft.mouseHandler.isMouseGrabbed();
	}
}
