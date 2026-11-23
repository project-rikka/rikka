package sh.rikka.client.mods.animation;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.model.HumanoidModel;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.state.ArmedEntityRenderState;
import net.minecraft.client.renderer.entity.state.HumanoidRenderState;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.state.level.FirstPersonHandsAndItemsRenderState;
import net.minecraft.client.renderer.state.level.PlayerRenderState;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Avatar;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.item.ItemStack;
import org.joml.Matrix4fc;
import org.jspecify.annotations.Nullable;

public final class AnimationHooks {
	private AnimationHooks() {
	}

	public static boolean firstPersonItem(
		PlayerRenderState playerState,
		FirstPersonHandsAndItemsRenderState state,
		float partialTicks,
		InteractionHand hand,
		float attack,
		ItemStack stack,
		float inverseArmHeight,
		PoseStack poseStack,
		SubmitNodeCollector collector,
		int lightCoords
	) {
		OldAnimationsMod mod = OldAnimationsMod.active();
		return mod != null
			&& FirstPersonHand.submit(mod, playerState, state, partialTicks, hand, attack, stack, inverseArmHeight, poseStack, collector, lightCoords);
	}

	public static @Nullable Matrix4fc thirdPersonItem(ArmedEntityRenderState state, ItemStackRenderState item, ItemStack stack, HumanoidArm arm) {
		OldAnimationsMod mod = OldAnimationsMod.active();
		return mod == null ? null : ThirdPersonItem.correction(mod, state, item, stack, arm);
	}

	public static HumanoidModel.ArmPose armPose(HumanoidModel.ArmPose pose, Avatar avatar, InteractionHand hand) {
		OldAnimationsMod mod = OldAnimationsMod.active();
		if (mod == null || pose != HumanoidModel.ArmPose.ITEM || hand != InteractionHand.MAIN_HAND || !(avatar instanceof LocalPlayer player)) {
			return pose;
		}
		return mod.blocking().isBlocking(player) ? HumanoidModel.ArmPose.BLOCK : pose;
	}

	public static boolean poseBlockingArm(HumanoidRenderState state, HumanoidArm arm, ModelPart part) {
		OldAnimationsMod mod = OldAnimationsMod.active();
		return mod != null && HumanoidPoses.blockingArm(mod, state, arm, part);
	}

	public static void sneakPose(HumanoidRenderState state, HumanoidModel<?> model) {
		OldAnimationsMod mod = OldAnimationsMod.active();
		if (mod != null) {
			HumanoidPoses.sneak(mod, state, model);
		}
	}

	public static float eyeHeight(Entity entity, float previous, float vanilla) {
		OldAnimationsMod mod = OldAnimationsMod.active();
		return mod == null || !mod.sneakCamera() ? vanilla : SneakCamera.eyeHeight(mod, entity, previous, vanilla);
	}

	public static boolean heartBlink(boolean blink) {
		OldAnimationsMod mod = OldAnimationsMod.active();
		return (mod == null || mod.heartFlash()) && blink;
	}

	public static int armorOverlay(Object state, int overlay) {
		OldAnimationsMod mod = OldAnimationsMod.active();
		if (mod == null || !mod.armorHitTint() || overlay != OverlayTexture.NO_OVERLAY) {
			return overlay;
		}
		if (state instanceof LivingEntityRenderState living && living.hasRedOverlay) {
			return LivingEntityRenderer.getOverlayCoords(living, 0.0F);
		}
		return overlay;
	}

	public static InteractionResult interacted(InteractionResult result) {
		OldAnimationsMod mod = OldAnimationsMod.active();
		if (mod != null && result instanceof InteractionResult.Success) {
			mod.blocking().spend();
		}
		return result;
	}
}
