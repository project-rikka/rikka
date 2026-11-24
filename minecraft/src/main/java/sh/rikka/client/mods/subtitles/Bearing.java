package sh.rikka.client.mods.subtitles;

import com.mojang.blaze3d.audio.ListenerTransform;
import net.minecraft.world.phys.Vec3;

enum Bearing {
	AHEAD("north"),
	AHEAD_RIGHT("north_east"),
	RIGHT("east"),
	BEHIND_RIGHT("south_east"),
	BEHIND("south"),
	BEHIND_LEFT("south_west"),
	LEFT("west"),
	AHEAD_LEFT("north_west"),
	AROUND("graphic_eq");

	private static final Bearing[] COMPASS = {AHEAD, AHEAD_RIGHT, RIGHT, BEHIND_RIGHT, BEHIND, BEHIND_LEFT, LEFT, AHEAD_LEFT};
	private static final double SECTOR = Math.PI * 2.0 / COMPASS.length;
	private static final double AROUND_DISTANCE_SQUARED = 2.0 * 2.0;

	private final String icon;

	Bearing(String icon) {
		this.icon = icon;
	}

	static Bearing of(ListenerTransform listener, Vec3 source) {
		Vec3 offset = source.subtract(listener.position());
		if (offset.lengthSqr() < AROUND_DISTANCE_SQUARED) {
			return AROUND;
		}
		double angle = Math.atan2(offset.dot(listener.right()), offset.dot(listener.forward()));
		return COMPASS[Math.floorMod((int) Math.round(angle / SECTOR), COMPASS.length)];
	}

	String icon() {
		return this.icon;
	}
}
