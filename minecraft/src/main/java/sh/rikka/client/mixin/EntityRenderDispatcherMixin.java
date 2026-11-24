package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import com.llamalad7.mixinextras.sugar.Local;
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.visual.VisualHooks;

@Mixin(EntityRenderDispatcher.class)
abstract class EntityRenderDispatcherMixin {
	@ModifyReturnValue(method = "extractEntity", at = @At("RETURN"))
	private EntityRenderState rikka$extracted(EntityRenderState state, @Local(argsOnly = true) Entity entity) {
		VisualHooks.entityExtracted(entity, state);
		return state;
	}

	@Inject(
		method = "submit",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/entity/EntityRenderer;submit(Lnet/minecraft/client/renderer/entity/state/EntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/state/level/CameraRenderState;)V",
			shift = At.Shift.AFTER
		)
	)
	private void rikka$submitted(
		EntityRenderState state,
		CameraRenderState camera,
		double x,
		double y,
		double z,
		PoseStack poseStack,
		SubmitNodeCollector submitNodeCollector,
		CallbackInfo callback
	) {
		VisualHooks.entitySubmitted(state, poseStack, submitNodeCollector);
	}
}
