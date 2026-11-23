package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import net.minecraft.client.CameraType;
import net.minecraft.client.Options;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.camera.CameraHooks;

@Mixin(Options.class)
abstract class OptionsMixin {
	@ModifyReturnValue(method = "getCameraType", at = @At("RETURN"))
	private CameraType rikka$cameraType(CameraType vanilla) {
		return CameraHooks.cameraType(vanilla);
	}
}
