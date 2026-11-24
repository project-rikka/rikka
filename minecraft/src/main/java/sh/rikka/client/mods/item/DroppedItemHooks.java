package sh.rikka.client.mods.item;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Axis;
import net.minecraft.client.renderer.entity.state.ItemEntityRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.phys.AABB;

public final class DroppedItemHooks {
	private static final float FLAT_DEPTH = 0.0625F;
	private static final float FLAT_STACK_SPACING = 1.5F;
	private static final float FLAT_REST_STEP = 180.0F;
	private static final float SOLID_REST_STEP = 90.0F;
	private static final float LYING = 90.0F;

	private DroppedItemHooks() {
	}

	public static void extracted(ItemEntity entity, ItemEntityRenderState state) {
		ItemPhysicsMod physics = ItemPhysicsMod.active();
		if (physics == null) {
			return;
		}
		float restStep = isFlat(state.item.getModelBoundingBox()) ? FLAT_REST_STEP : SOLID_REST_STEP;
		float tumble = ((TumblingItem) entity).rikka$tumble().advance(state.ageInTicks, entity.onGround(), entity.isInLiquid(), physics.speed(), restStep);
		((DroppedItemState) state).rikka$setTumble(tumble);
	}

	public static boolean place(PoseStack poseStack, ItemEntityRenderState state, AABB box) {
		if (ItemPhysicsMod.active() == null) {
			return false;
		}
		boolean flat = isFlat(box);
		float tilt = ((DroppedItemState) state).rikka$tumble() + (flat ? LYING : 0.0F);
		float radians = tilt * Mth.DEG_TO_RAD;
		float halfHeight = (float) box.getYsize() / 2.0F;
		float halfDepth = depth(box, flat, state.count) / 2.0F;
		float lift = Math.abs(Mth.cos(radians)) * halfHeight + Math.abs(Mth.sin(radians)) * halfDepth;
		poseStack.translate(0.0F, lift, 0.0F);
		poseStack.rotate(Axis.YP, state.bobOffset);
		poseStack.rotate(Axis.XP, radians);
		poseStack.translate(-(float) box.getCenter().x, -(float) box.getCenter().y, -(float) box.getCenter().z);
		return true;
	}

	public static boolean spin(PoseStack poseStack, ItemEntityRenderState state, CameraRenderState camera, AABB box) {
		if (ItemPhysicsMod.active() != null) {
			return true;
		}
		if (Items2DMod.active() != null && isFlat(box)) {
			poseStack.rotate(camera.orientation);
			return true;
		}
		return false;
	}

	private static float depth(AABB box, boolean flat, int count) {
		float depth = (float) box.getZsize();
		return flat ? depth + depth * FLAT_STACK_SPACING * Math.max(0, count - 1) : depth;
	}

	private static boolean isFlat(AABB box) {
		return box.getZsize() <= FLAT_DEPTH;
	}
}
