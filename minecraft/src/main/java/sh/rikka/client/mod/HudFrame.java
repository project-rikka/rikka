package sh.rikka.client.mod;

import net.minecraft.client.gui.GuiGraphicsExtractor;

public record HudFrame(GuiGraphicsExtractor graphics, float partialTick, float screenWidth, float screenHeight, int guiScale) {
}
