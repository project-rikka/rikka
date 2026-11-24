package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Axis;
import net.minecraft.client.renderer.entity.ItemEntityRenderer;
import net.minecraft.client.renderer.entity.state.ItemEntityRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.phys.AABB;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.item.DroppedItemHooks;

@Mixin(ItemEntityRenderer.class)
abstract class ItemEntityRendererMixin {
	@Inject(
		method = "extractRenderState(Lnet/minecraft/world/entity/item/ItemEntity;Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;F)V",
		at = @At("TAIL")
	)
	private void rikka$extracted(ItemEntity entity, ItemEntityRenderState state, float partialTicks, CallbackInfo callback) {
		DroppedItemHooks.extracted(entity, state);
	}

	@WrapOperation(
		method = "submit(Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/state/level/CameraRenderState;)V",
		at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/vertex/PoseStack;translate(FFF)V", ordinal = 0)
	)
	private void rikka$place(
		PoseStack poseStack,
		float x,
		float y,
		float z,
		Operation<Void> original,
		@Local(argsOnly = true) ItemEntityRenderState state,
		@Local AABB box
	) {
		if (!DroppedItemHooks.place(poseStack, state, box)) {
			original.call(poseStack, x, y, z);
		}
	}

	@WrapOperation(
		method = "submit(Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/state/level/CameraRenderState;)V",
		at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/vertex/PoseStack;rotate(Lcom/mojang/math/Axis;F)V", ordinal = 0)
	)
	private void rikka$spin(
		PoseStack poseStack,
		Axis axis,
		float angle,
		Operation<Void> original,
		@Local(argsOnly = true) ItemEntityRenderState state,
		@Local(argsOnly = true) CameraRenderState camera,
		@Local AABB box
	) {
		if (!DroppedItemHooks.spin(poseStack, state, camera, box)) {
			original.call(poseStack, axis, angle);
		}
	}
}
