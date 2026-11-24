package sh.rikka.client.mods.animation;

import net.minecraft.client.model.HumanoidModel;
import net.minecraft.client.renderer.entity.state.ArmedEntityRenderState;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.ItemStack;
import org.joml.Matrix4fc;
import org.jspecify.annotations.Nullable;

final class ThirdPersonItem {
	private ThirdPersonItem() {
	}

	static @Nullable Matrix4fc correction(
		OldAnimationsMod mod, ArmedEntityRenderState state, ItemStackRenderState item, ItemStack stack, HumanoidArm arm
	) {
		if (!(state instanceof AvatarRenderState avatar) || avatar.isBaby) {
			return null;
		}
		HumanoidModel.ArmPose pose = avatar.getArmPose(arm);
		if (pose == HumanoidModel.ArmPose.SPEAR || HeldItems.isStabbing(avatar.currentSwing)) {
			return null;
		}
		boolean blocking = isLegacyBlock(mod, pose, stack);
		if (arm == HumanoidArm.LEFT) {
			return blocking ? ThirdPersonPose.blockInsert(true) : null;
		}
		DisplayedItem displayed = mod.thirdPersonItems() ? DisplayedItem.of(item, ItemDisplayContext.THIRD_PERSON_RIGHT_HAND) : null;
		if (displayed != null && (!blocking || displayed.family() == ItemFamily.HANDHELD)) {
			return ThirdPersonPose.exact(displayed.family(), blocking, displayed.display());
		}
		return blocking ? ThirdPersonPose.blockInsert(false) : null;
	}

	private static boolean isLegacyBlock(OldAnimationsMod mod, HumanoidModel.ArmPose pose, ItemStack stack) {
		return pose == HumanoidModel.ArmPose.BLOCK && !HeldItems.isShield(stack) && mod.thirdPersonBlock() == ThirdPersonBlock.V1_7;
	}
}
