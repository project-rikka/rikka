package sh.rikka.client.mods.animation;

import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.entity.player.Player;

final class SneakCamera {
	private static final float RISE_FRACTION = 0.6F;
	private static final float LOWER_POSE_FRACTION = 0.5F;
	private static final float EPSILON = 1.0E-3F;

	private SneakCamera() {
	}

	static float eyeHeight(OldAnimationsMod mod, Entity entity, float previous, float vanilla) {
		if (!(entity instanceof Player player)) {
			return vanilla;
		}
		float standing = player.getEyeHeight(Pose.STANDING);
		float crouched = standing - mod.sneakDepth();
		if (player.isCrouching()) {
			return previous >= crouched ? crouched : previous + (crouched - previous) * LOWER_POSE_FRACTION;
		}
		Pose pose = player.getPose();
		boolean rising = (pose == Pose.STANDING || pose == Pose.CROUCHING) && previous < standing && previous >= crouched - EPSILON;
		if (!rising) {
			return vanilla;
		}
		float next = mod.sneakEase() ? previous + (standing - previous) * RISE_FRACTION : standing;
		return standing - next < EPSILON ? standing : next;
	}
}
