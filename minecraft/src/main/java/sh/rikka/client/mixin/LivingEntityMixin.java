package sh.rikka.client.mixin;

import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;
import sh.rikka.client.mods.mechanic.MechanicHooks;

@Mixin(LivingEntity.class)
abstract class LivingEntityMixin {
	@Shadow
	private int noJumpDelay;

	@Inject(method = "jumpFromGround", at = @At("HEAD"))
	private void rikka$jumpFromGround(CallbackInfo callback) {
		Object self = this;
		if (self instanceof LocalPlayer player) {
			ClientHooks.localPlayerJump(player);
		}
	}

	@Inject(method = "aiStep", at = @At("HEAD"))
	private void rikka$noJumpDelay(CallbackInfo callback) {
		LivingEntity self = (LivingEntity) (Object) this;
		if (MechanicHooks.clearsJumpDelay(self)) {
			this.noJumpDelay = 0;
		}
	}
}
