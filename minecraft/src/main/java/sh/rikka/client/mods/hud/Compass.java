package sh.rikka.client.mods.hud;

import java.util.Locale;
import net.minecraft.util.Mth;

enum Compass {
	SOUTH,
	SOUTH_WEST,
	WEST,
	NORTH_WEST,
	NORTH,
	NORTH_EAST,
	EAST,
	SOUTH_EAST;

	private static final Compass[] POINTS = values();
	private static final float DEGREES_PER_POINT = 360.0F / POINTS.length;

	private final String translationSuffix = "direction." + name().toLowerCase(Locale.ROOT);

	static Compass fromYaw(float yaw) {
		return POINTS[Math.floorMod(Mth.floor(yaw / DEGREES_PER_POINT + 0.5F), POINTS.length)];
	}

	String translationSuffix() {
		return this.translationSuffix;
	}
}
