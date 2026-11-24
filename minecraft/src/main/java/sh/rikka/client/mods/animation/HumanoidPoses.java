package sh.rikka.client.mods.animation;

import net.minecraft.client.model.HumanoidModel;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.renderer.entity.state.HumanoidRenderState;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.item.ItemStack;

final class HumanoidPoses {
	private static final float BLOCKING_ARM_PITCH = 0.9424779F;
	private static final float BLOCKING_ARM_YAW = (float) (Math.PI / 6);
	private static final float SNEAK_BODY_SHIFT = 3.2F;
	private static final float SNEAK_HEAD_SHIFT = 4.2F;
	private static final float SNEAK_HEAD_HEIGHT = 1.0F;
	private static final float SNEAK_LEG_RAISE = 3.0F;

	private HumanoidPoses() {
	}

	static boolean blockingArm(OldAnimationsMod mod, HumanoidRenderState state, HumanoidArm arm, ModelPart part) {
		if (state.getArmPose(arm) != HumanoidModel.ArmPose.BLOCK) {
			return false;
		}
		ItemStack stack = arm == HumanoidArm.RIGHT ? state.rightHandItemStack : state.leftHandItemStack;
		ThirdPersonBlock style = mod.thirdPersonBlock();
		if (style == ThirdPersonBlock.MODERN || HeldItems.isShield(stack)) {
			return false;
		}
		part.xRot = part.xRot * 0.5F - BLOCKING_ARM_PITCH;
		float yaw = style == ThirdPersonBlock.V1_8 ? BLOCKING_ARM_YAW : 0.0F;
		part.yRot = arm == HumanoidArm.RIGHT ? -yaw : yaw;
		return true;
	}

	static void sneak(OldAnimationsMod mod, HumanoidRenderState state, HumanoidModel<?> model) {
		if (!mod.sneakPose() || !state.isCrouching) {
			return;
		}
		model.body.y -= SNEAK_BODY_SHIFT;
		model.rightArm.y -= SNEAK_BODY_SHIFT;
		model.leftArm.y -= SNEAK_BODY_SHIFT;
		model.head.y += SNEAK_HEAD_HEIGHT - SNEAK_HEAD_SHIFT;
		model.rightLeg.y -= SNEAK_LEG_RAISE;
		model.leftLeg.y -= SNEAK_LEG_RAISE;
	}
}
