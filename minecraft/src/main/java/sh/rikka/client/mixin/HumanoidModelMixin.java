package sh.rikka.client.mixin;

import net.minecraft.client.model.HumanoidModel;
import net.minecraft.client.renderer.entity.state.HumanoidRenderState;
import net.minecraft.world.entity.HumanoidArm;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.animation.AnimationHooks;

@Mixin(HumanoidModel.class)
abstract class HumanoidModelMixin<T extends HumanoidRenderState> {
	@Inject(method = "poseRightArm", at = @At("HEAD"), cancellable = true)
	private void rikka$poseRightArm(T state, CallbackInfo callback) {
		HumanoidModel<?> model = (HumanoidModel<?>) (Object) this;
		if (AnimationHooks.poseBlockingArm(state, HumanoidArm.RIGHT, model.rightArm)) {
			callback.cancel();
		}
	}

	@Inject(method = "poseLeftArm", at = @At("HEAD"), cancellable = true)
	private void rikka$poseLeftArm(T state, CallbackInfo callback) {
		HumanoidModel<?> model = (HumanoidModel<?>) (Object) this;
		if (AnimationHooks.poseBlockingArm(state, HumanoidArm.LEFT, model.leftArm)) {
			callback.cancel();
		}
	}

	@Inject(method = "setupAnim(Lnet/minecraft/client/renderer/entity/state/HumanoidRenderState;)V", at = @At("RETURN"))
	private void rikka$sneakPose(T state, CallbackInfo callback) {
		AnimationHooks.sneakPose(state, (HumanoidModel<?>) (Object) this);
	}
}
