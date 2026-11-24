package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import net.minecraft.client.ClientClockManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.visual.OverworldClock;
import sh.rikka.client.mods.visual.VisualHooks;

@Mixin(ClientClockManager.ClientClockInstance.class)
abstract class ClientClockInstanceMixin implements OverworldClock {
	@Unique
	private boolean rikka$overworld;

	@Override
	public void rikka$setOverworld(boolean overworld) {
		this.rikka$overworld = overworld;
	}

	@ModifyReturnValue(method = "totalTicks", at = @At("RETURN"))
	private long rikka$totalTicks(long vanilla) {
		return this.rikka$overworld ? VisualHooks.overworldClockTicks(vanilla) : vanilla;
	}
}
