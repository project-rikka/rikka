package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.world.level.Level;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.weather.WeatherHooks;

@Mixin(Level.class)
abstract class LevelMixin {
	@ModifyReturnValue(method = "getRainLevel", at = @At("RETURN"))
	private float rikka$rainLevel(float vanilla, @Local(argsOnly = true) float partialTick) {
		return WeatherHooks.rainLevel((Level) (Object) this, partialTick, vanilla);
	}

	@ModifyReturnValue(method = "getThunderLevel", at = @At("RETURN"))
	private float rikka$thunderLevel(float vanilla, @Local(argsOnly = true) float partialTick) {
		return WeatherHooks.thunderLevel((Level) (Object) this, partialTick, vanilla);
	}
}
