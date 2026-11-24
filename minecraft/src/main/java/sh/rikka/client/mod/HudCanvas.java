package sh.rikka.client.mod;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.PlayerFaceExtractor;
import net.minecraft.client.gui.screens.inventory.InventoryScreen;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.PlayerSkin;
import net.minecraft.world.item.ItemStack;
import org.joml.Matrix3x2fStack;
import org.joml.Vector2f;

public final class HudCanvas {
	private static final float ITEM_SIZE = 16.0F;
	private static final float FACE_SIZE = 8.0F;
	private static final float ENTITY_FILL = 0.8F;

	private final HudFrame frame;
	private final float pixel;

	HudCanvas(HudFrame frame, float pixel) {
		this.frame = frame;
		this.pixel = pixel;
	}

	public GuiGraphicsExtractor graphics() {
		return this.frame.graphics();
	}

	public float partialTick() {
		return this.frame.partialTick();
	}

	public float pixel() {
		return this.pixel;
	}

	public void item(ItemStack stack, float x, float y, float size) {
		if (stack.isEmpty()) {
			return;
		}
		GuiGraphicsExtractor graphics = graphics();
		Matrix3x2fStack pose = graphics.pose();
		pose.pushMatrix();
		try {
			pose.translate(x, y);
			pose.scale(size / ITEM_SIZE);
			graphics.item(stack, 0, 0);
			graphics.itemDecorations(Minecraft.getInstance().font, stack, 0, 0);
		} finally {
			pose.popMatrix();
		}
	}

	public void face(PlayerSkin skin, float x, float y, float size, int argb) {
		GuiGraphicsExtractor graphics = graphics();
		Matrix3x2fStack pose = graphics.pose();
		pose.pushMatrix();
		try {
			pose.translate(x, y);
			pose.scale(size / FACE_SIZE);
			PlayerFaceExtractor.extractRenderState(graphics, skin, 0, 0, (int) FACE_SIZE, argb);
		} finally {
			pose.popMatrix();
		}
	}

	public void entity(LivingEntity entity, float x, float y, float width, float height) {
		Matrix3x2fStack pose = graphics().pose();
		Vector2f topLeft = pose.transformPosition(x, y, new Vector2f());
		Vector2f bottomRight = pose.transformPosition(x + width, y + height, new Vector2f());
		int x0 = Math.round(topLeft.x);
		int y0 = Math.round(topLeft.y);
		int x1 = Math.round(bottomRight.x);
		int y1 = Math.round(bottomRight.y);
		if (x1 <= x0 || y1 <= y0) {
			return;
		}
		float blocks = Math.max(entity.getBbHeight(), entity.getBbWidth());
		int size = Math.max(1, Math.round(Math.min(x1 - x0, y1 - y0) * ENTITY_FILL / blocks));
		float centerX = (x0 + x1) / 2.0F;
		float centerY = (y0 + y1) / 2.0F;
		InventoryScreen.extractEntityInInventoryFollowsMouse(graphics(), x0, y0, x1, y1, size, 0.0F, centerX, centerY, entity);
	}
}
