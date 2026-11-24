package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.layers.ItemInHandLayer;
import net.minecraft.client.renderer.entity.state.ArmedEntityRenderState;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.item.ItemStack;
import org.joml.Matrix4fc;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.animation.AnimationHooks;

@Mixin(ItemInHandLayer.class)
abstract class ItemInHandLayerMixin<S extends ArmedEntityRenderState> {
	@WrapOperation(
		method = "submitArmWithItem",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/item/ItemStackRenderState;submit(Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;III)V"
		)
	)
	private void rikka$legacyItem(
		ItemStackRenderState item,
		PoseStack poseStack,
		SubmitNodeCollector collector,
		int lightCoords,
		int overlayCoords,
		int outlineColor,
		Operation<Void> original,
		@Local(argsOnly = true) S state,
		@Local(argsOnly = true) ItemStack itemStack,
		@Local(argsOnly = true) HumanoidArm arm
	) {
		Matrix4fc correction = AnimationHooks.thirdPersonItem(state, item, itemStack, arm);
		if (correction == null) {
			original.call(item, poseStack, collector, lightCoords, overlayCoords, outlineColor);
			return;
		}
		poseStack.pushPose();
		poseStack.mulPose(correction);
		original.call(item, poseStack, collector, lightCoords, overlayCoords, outlineColor);
		poseStack.popPose();
	}
}
