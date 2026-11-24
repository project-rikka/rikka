package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.player.KeyboardInput;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.mechanic.MechanicHooks;

@Mixin(KeyboardInput.class)
abstract class KeyboardInputMixin {
	@WrapOperation(method = "tick", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/KeyMapping;isDown()Z"))
	private boolean rikka$isDown(KeyMapping key, Operation<Boolean> original) {
		return MechanicHooks.keyDown(key, original.call(key));
	}
}
