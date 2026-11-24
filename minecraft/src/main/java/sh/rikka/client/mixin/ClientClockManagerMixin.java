package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.ClientClockManager;
import net.minecraft.core.Holder;
import net.minecraft.world.clock.WorldClock;
import net.minecraft.world.clock.WorldClocks;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.visual.OverworldClock;

@Mixin(ClientClockManager.class)
abstract class ClientClockManagerMixin {
	@ModifyReturnValue(
		method = "getInstance(Lnet/minecraft/core/Holder;)Lnet/minecraft/client/ClientClockManager$ClientClockInstance;",
		at = @At("RETURN")
	)
	private ClientClockManager.ClientClockInstance rikka$markOverworld(
		ClientClockManager.ClientClockInstance instance, @Local(argsOnly = true) Holder<WorldClock> definition
	) {
		((OverworldClock) instance).rikka$setOverworld(definition.is(WorldClocks.OVERWORLD));
		return instance;
	}
}
