package sh.rikka.client.render.entity;

import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.util.Util;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mods.boost.BoostFeature;

public final class DistantEntities {
	private static final double NEAR_BLOCKS = 24.0;
	private static final double NEAR_BLOCKS_SQUARED = NEAR_BLOCKS * NEAR_BLOCKS;
	private static final long REFRESH_NANOS = 20_000_000L;
	private static final int PHASES = 8;
	private static final long PHASE_NANOS = REFRESH_NANOS / (2 * PHASES);

	private DistantEntities() {
	}

	public static @Nullable EntityRenderState retained(Entity entity, double cameraX, double cameraY, double cameraZ) {
		if (!BoostFeature.DISTANT_ENTITIES.isActive()
			|| !(entity instanceof LivingEntity living)
			|| entity.isPassenger()
			|| entity.isVehicle()
			|| entity.distanceToSqr(cameraX, cameraY, cameraZ) <= NEAR_BLOCKS_SQUARED) {
			return null;
		}
		RetainedEntityState retained = (RetainedEntityState) living;
		long age = Util.getNanos() - retained.rikka$retainedAt();
		return age < REFRESH_NANOS + Math.floorMod(entity.getId(), PHASES) * PHASE_NANOS ? retained.rikka$retainedState() : null;
	}

	public static void retain(Entity entity, EntityRenderState state) {
		if (BoostFeature.DISTANT_ENTITIES.isActive() && entity instanceof LivingEntity living) {
			((RetainedEntityState) living).rikka$retainState(state, Util.getNanos());
		}
	}
}
