package sh.rikka.client.mods.minimap;

import net.minecraft.util.Mth;
import sh.rikka.client.render.GuiMesh;

public enum MapShape {
	WIDE,
	SQUARE,
	CIRCLE;

	private static final float WIDE_ASPECT = 9.0F / 16.0F;
	private static final float CORNER_FRACTION = 0.12F;
	private static final int CORNER_SEGMENTS = 8;

	float aspect() {
		return switch (this) {
			case WIDE -> WIDE_ASPECT;
			case SQUARE, CIRCLE -> 1.0F;
		};
	}

	float cornerRadius(float width, float height) {
		return switch (this) {
			case CIRCLE -> width / 2.0F;
			case SQUARE, WIDE -> Math.min(width, height) * CORNER_FRACTION;
		};
	}

	float[] outline(float centerX, float centerY, float halfWidth, float halfHeight, float feather) {
		return switch (this) {
			case CIRCLE -> GuiMesh.circle(centerX, centerY, halfWidth, GuiMesh.segments(halfWidth, feather));
			case SQUARE, WIDE -> roundedRectangle(centerX, centerY, halfWidth, halfHeight, cornerRadius(halfWidth * 2.0F, halfHeight * 2.0F));
		};
	}

	boolean contains(float x, float y, float halfWidth, float halfHeight) {
		return switch (this) {
			case CIRCLE -> x * x + y * y <= halfWidth * halfWidth;
			case SQUARE, WIDE -> Math.abs(x) <= halfWidth && Math.abs(y) <= halfHeight;
		};
	}

	float reach(float halfWidth, float halfHeight) {
		return switch (this) {
			case CIRCLE -> halfWidth;
			case SQUARE, WIDE -> Mth.sqrt(halfWidth * halfWidth + halfHeight * halfHeight);
		};
	}

	float rimDistance(float directionX, float directionY, float halfWidth, float halfHeight) {
		return switch (this) {
			case CIRCLE -> halfWidth;
			case SQUARE, WIDE -> Math.min(
				Math.abs(directionX) > 0.0F ? halfWidth / Math.abs(directionX) : Float.POSITIVE_INFINITY,
				Math.abs(directionY) > 0.0F ? halfHeight / Math.abs(directionY) : Float.POSITIVE_INFINITY
			);
		};
	}

	private static float[] roundedRectangle(float centerX, float centerY, float halfWidth, float halfHeight, float radius) {
		float[] outline = new float[4 * (CORNER_SEGMENTS + 1) * 2];
		float innerX = halfWidth - radius;
		float innerY = halfHeight - radius;
		int[][] corners = {{1, 1}, {-1, 1}, {-1, -1}, {1, -1}};
		int index = 0;
		for (int corner = 0; corner < corners.length; corner++) {
			float cornerX = centerX + corners[corner][0] * innerX;
			float cornerY = centerY + corners[corner][1] * innerY;
			float start = corner * Mth.HALF_PI;
			for (int step = 0; step <= CORNER_SEGMENTS; step++) {
				float angle = start + Mth.HALF_PI * step / CORNER_SEGMENTS;
				outline[index++] = cornerX + Mth.cos(angle) * radius;
				outline[index++] = cornerY + Mth.sin(angle) * radius;
			}
		}
		return outline;
	}
}
