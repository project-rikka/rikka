package sh.rikka.bench.mixin;

import net.minecraft.client.renderer.LightmapRenderStateExtractor;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(LightmapRenderStateExtractor.class)
abstract class LightmapRenderStateExtractorMixin {
	@Inject(method = "tick", at = @At("HEAD"), cancellable = true)
	private void bench$steadyLight(CallbackInfo callback) {
		callback.cancel();
	}
}
