package sh.rikka.client.mixin;

import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;

@Mixin(LivingEntity.class)
abstract class LivingEntityMixin {
	@Inject(method = "jumpFromGround", at = @At("HEAD"))
	private void rikka$jumpFromGround(CallbackInfo callback) {
		Object self = this;
		if (self instanceof LocalPlayer player) {
			ClientHooks.localPlayerJump(player);
		}
	}
}
