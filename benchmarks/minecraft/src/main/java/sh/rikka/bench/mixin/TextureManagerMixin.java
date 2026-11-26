package sh.rikka.bench.mixin;

import net.minecraft.client.renderer.texture.TextureManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.bench.BenchMod;

@Mixin(TextureManager.class)
abstract class TextureManagerMixin {
	@Inject(method = "tick", at = @At("HEAD"), cancellable = true)
	private void bench$holdAnimations(CallbackInfo callback) {
		if (!BenchMod.animatesTextures()) {
			callback.cancel();
		}
	}
}
