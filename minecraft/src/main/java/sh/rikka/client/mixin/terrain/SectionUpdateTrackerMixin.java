package sh.rikka.client.mixin.terrain;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.RotatingSectionStorage;
import net.minecraft.client.SectionUpdateTracker;
import net.minecraft.core.SectionPos;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.render.terrain.DirtyStateEpoch;

@Mixin(SectionUpdateTracker.class)
abstract class SectionUpdateTrackerMixin {
	@Inject(method = "<init>", at = @At("TAIL"))
	private void rikka$created(CallbackInfo callback) {
		DirtyStateEpoch.advance();
	}

	@WrapOperation(
		method = "repositionCamera",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/client/RotatingSectionStorage;repositionCenter(Lnet/minecraft/core/SectionPos;)Z")
	)
	private boolean rikka$repositioned(RotatingSectionStorage<?> storage, SectionPos center, Operation<Boolean> original) {
		boolean moved = original.call(storage, center);
		if (moved) {
			DirtyStateEpoch.advance();
		}
		return moved;
	}
}
