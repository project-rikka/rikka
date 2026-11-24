package sh.rikka.client.hud;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import sh.rikka.client.setting.Anchor;
import sh.rikka.client.setting.ScreenPosition;

final class Snapping {
	private static final float THRESHOLD = 6.0F;
	private static final float MARGIN = 4.0F;
	private static final float THIRD = 1.0F / 3.0F;
	private static final Anchor[][] ANCHORS = {
		{Anchor.TOP_LEFT, Anchor.TOP_CENTER, Anchor.TOP_RIGHT},
		{Anchor.CENTER_LEFT, Anchor.CENTER, Anchor.CENTER_RIGHT},
		{Anchor.BOTTOM_LEFT, Anchor.BOTTOM_CENTER, Anchor.BOTTOM_RIGHT},
	};

	private Snapping() {
	}

	record Snap(float left, float top, Optional<Float> guideX, Optional<Float> guideY) {
		static Snap free(float left, float top) {
			return new Snap(left, top, Optional.empty(), Optional.empty());
		}
	}

	private record Line(float start, Optional<Float> guide) {
	}

	static Snap snap(HudBox moving, float left, float top, List<HudBox> others, float screenWidth, float screenHeight) {
		List<Float> columns = new ArrayList<>(List.of(0.0F, MARGIN, screenWidth / 2.0F, screenWidth - MARGIN, screenWidth));
		List<Float> rows = new ArrayList<>(List.of(0.0F, MARGIN, screenHeight / 2.0F, screenHeight - MARGIN, screenHeight));
		for (HudBox other : others) {
			columns.addAll(List.of(other.left(), other.centerX(), other.right()));
			rows.addAll(List.of(other.top(), other.centerY(), other.bottom()));
		}
		Line horizontal = align(left, moving.width(), screenWidth, columns);
		Line vertical = align(top, moving.height(), screenHeight, rows);
		return new Snap(horizontal.start(), vertical.start(), horizontal.guide(), vertical.guide());
	}

	static Snap clamp(HudBox moving, float left, float top, float screenWidth, float screenHeight) {
		return Snap.free(clamp(left, moving.width(), screenWidth), clamp(top, moving.height(), screenHeight));
	}

	static ScreenPosition position(HudBox moving, float left, float top, float screenWidth, float screenHeight) {
		float roomX = Math.max(0.0F, screenWidth - moving.width());
		float roomY = Math.max(0.0F, screenHeight - moving.height());
		Anchor anchor = ANCHORS[third(top + moving.height() / 2.0F, screenHeight)][third(left + moving.width() / 2.0F, screenWidth)];
		return new ScreenPosition(anchor, left - anchor.horizontal() * roomX, top - anchor.vertical() * roomY);
	}

	private static int third(float center, float screen) {
		float share = screen > 0.0F ? center / screen : 0.0F;
		return share < THIRD ? 0 : share < 1.0F - THIRD ? 1 : 2;
	}

	private static Line align(float start, float size, float screen, List<Float> lines) {
		float[] edges = {start, start + size / 2.0F, start + size};
		Optional<Float> guide = Optional.empty();
		float shift = 0.0F;
		float nearest = THRESHOLD;
		for (float edge : edges) {
			for (float line : lines) {
				float distance = line - edge;
				if (Math.abs(distance) <= nearest) {
					nearest = Math.abs(distance);
					shift = distance;
					guide = Optional.of(line);
				}
			}
		}
		return new Line(clamp(start + shift, size, screen), guide);
	}

	private static float clamp(float start, float size, float screen) {
		return Math.clamp(start, 0.0F, Math.max(0.0F, screen - size));
	}
}
