package sh.rikka.client.mixin;

import net.minecraft.client.KeyMapping;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.mechanic.MechanicHooks;

@Mixin(KeyMapping.class)
abstract class KeyMappingMixin {
	@Shadow
	private boolean isDown;

	@Inject(method = "setDown", at = @At("HEAD"))
	private void rikka$setDown(boolean down, CallbackInfo callback) {
		if (down && !this.isDown) {
			MechanicHooks.keyPressed((KeyMapping) (Object) this);
		}
	}
}
