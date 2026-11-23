package sh.rikka.client.mixin;

import net.minecraft.client.Camera;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.Level;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.animation.AnimationHooks;

@Mixin(Camera.class)
abstract class CameraMixin {
	@Shadow
	private @Nullable Level level;
	@Shadow
	private @Nullable Entity entity;
	@Shadow
	private float eyeHeight;
	@Shadow
	private float eyeHeightOld;

	@Inject(method = "tick", at = @At("RETURN"))
	private void rikka$sneakEyeHeight(CallbackInfo callback) {
		if (this.level != null && this.entity != null) {
			this.eyeHeight = AnimationHooks.eyeHeight(this.entity, this.eyeHeightOld, this.eyeHeight);
		}
	}
}
