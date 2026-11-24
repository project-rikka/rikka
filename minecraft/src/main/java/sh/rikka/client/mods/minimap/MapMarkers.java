package sh.rikka.client.mods.minimap;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.MobCategory;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix3x2fStack;
import sh.rikka.client.mod.HudCanvas;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.render.GuiMesh;

final class MapMarkers {
	private static final float DOT_RADIUS = 2.4F;
	private static final float DOT_OUTLINE = 1.0F;
	private static final float HEAD_SIZE = 9.0F;
	private static final float HEAD_BORDER = 1.0F;
	private static final float ARROW_SCALE = 1.0F;
	private static final float ARROW_OUTLINE_SCALE = 1.45F;
	private static final float CARDINAL_INSET = 7.0F;
	private static final int HOSTILE_COLOR = 0xFFFF5A5A;
	private static final int ANIMAL_COLOR = 0xFF8BE36B;
	private static final int OUTLINE_COLOR = 0xB0000000;
	private static final int ARROW_COLOR = 0xFFFFFFFF;
	private static final int NORTH_COLOR = 0xFFFF6B6B;
	private static final int CARDINAL_COLOR = 0xFFFFFFFF;
	private static final float[] ARROW = {0.0F, -7.0F, 5.0F, 6.0F, 0.0F, 3.0F, -5.0F, 6.0F};
	private static final float[][] DIRECTIONS = {{0.0F, -1.0F}, {1.0F, 0.0F}, {0.0F, 1.0F}, {-1.0F, 0.0F}};

	private MapMarkers() {
	}

	static void entities(HudCanvas canvas, MapProjection projection, ClientLevel level, LocalPlayer self, Selection selection) {
		float partialTick = canvas.partialTick();
		GuiMesh dots = GuiMesh.colored(canvas.graphics());
		for (Entity entity : level.entitiesForRendering()) {
			if (entity == self || entity.isInvisibleTo(self) || entity instanceof AbstractClientPlayer) {
				continue;
			}
			int color = dotColor(entity, selection);
			if (color == 0) {
				continue;
			}
			Vec3 position = entity.getPosition(partialTick);
			float x = projection.screenX(position.x, position.z);
			float y = projection.screenY(position.x, position.z);
			if (projection.contains(x, y, DOT_RADIUS + DOT_OUTLINE)) {
				dots.disc(x, y, DOT_RADIUS + DOT_OUTLINE, OUTLINE_COLOR, canvas.pixel());
				dots.disc(x, y, DOT_RADIUS, color, canvas.pixel());
			}
		}
		dots.submit(canvas.graphics());
		if (!selection.players()) {
			return;
		}
		for (AbstractClientPlayer player : level.players()) {
			if (player == self || player.isInvisibleTo(self)) {
				continue;
			}
			Vec3 position = player.getPosition(partialTick);
			float x = projection.screenX(position.x, position.z);
			float y = projection.screenY(position.x, position.z);
			float half = HEAD_SIZE / 2.0F;
			if (!projection.contains(x, y, half + HEAD_BORDER)) {
				continue;
			}
			GuiMesh.colored(canvas.graphics()).rect(x - half - HEAD_BORDER, y - half - HEAD_BORDER, x + half + HEAD_BORDER, y + half + HEAD_BORDER, OUTLINE_COLOR).submit(canvas.graphics());
			canvas.face(player.getSkin(), x - half, y - half, HEAD_SIZE, 0xFFFFFFFF);
		}
	}

	static void arrow(HudCanvas canvas, MapProjection projection, float heading) {
		GuiMesh mesh = GuiMesh.colored(canvas.graphics());
		chevron(mesh, projection, heading, ARROW_SCALE * ARROW_OUTLINE_SCALE, OUTLINE_COLOR);
		chevron(mesh, projection, heading, ARROW_SCALE, ARROW_COLOR);
		mesh.submit(canvas.graphics());
	}

	static void cardinals(HudCanvas canvas, MapProjection projection, String[] labels) {
		GuiGraphicsExtractor graphics = canvas.graphics();
		Font font = Minecraft.getInstance().font;
		float cos = Mth.cos(projection.rotation());
		float sin = Mth.sin(projection.rotation());
		for (int i = 0; i < DIRECTIONS.length; i++) {
			float worldX = DIRECTIONS[i][0];
			float worldZ = DIRECTIONS[i][1];
			float screenX = cos * worldX + sin * worldZ;
			float screenY = -sin * worldX + cos * worldZ;
			float distance = projection.rimDistance(screenX, screenY) - CARDINAL_INSET;
			float x = projection.centerX() + screenX * distance;
			float y = projection.centerY() + screenY * distance;
			Matrix3x2fStack pose = graphics.pose();
			pose.pushMatrix();
			try {
				pose.translate(x, y);
				pose.scale(HudMod.UNITS_PER_GUI_PIXEL);
				String label = labels[i];
				graphics.text(font, label, -font.width(label) / 2, -font.lineHeight / 2, i == 0 ? NORTH_COLOR : CARDINAL_COLOR, true);
			} finally {
				pose.popMatrix();
			}
		}
	}

	private static void chevron(GuiMesh mesh, MapProjection projection, float heading, float scale, int argb) {
		float cos = Mth.cos(heading);
		float sin = Mth.sin(heading);
		float[] points = new float[ARROW.length];
		for (int i = 0; i < ARROW.length; i += 2) {
			float x = ARROW[i] * scale;
			float y = ARROW[i + 1] * scale;
			points[i] = projection.centerX() + cos * x - sin * y;
			points[i + 1] = projection.centerY() + sin * x + cos * y;
		}
		mesh.triangle(points[0], points[1], points[2], points[3], points[4], points[5], argb);
		mesh.triangle(points[0], points[1], points[4], points[5], points[6], points[7], argb);
	}

	private static int dotColor(Entity entity, Selection selection) {
		if (!(entity instanceof LivingEntity)) {
			return 0;
		}
		MobCategory category = entity.getType().getCategory();
		if (category == MobCategory.MONSTER) {
			return selection.hostiles() ? HOSTILE_COLOR : 0;
		}
		if (category == MobCategory.MISC) {
			return 0;
		}
		return selection.animals() ? ANIMAL_COLOR : 0;
	}

	record Selection(boolean players, boolean hostiles, boolean animals) {
	}
}
