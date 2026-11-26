package sh.rikka.bench.mixin;

import net.minecraft.client.renderer.SkyRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.bench.BenchMod;
import sh.rikka.bench.GpuSpan;

@Mixin(SkyRenderer.class)
abstract class SkyRendererMixin {
	@Inject(method = "render", at = @At("HEAD"))
	private void bench$skyStarting(CallbackInfo callback) {
		BenchMod.gpuBegin(GpuSpan.SKY, null);
	}

	@Inject(method = "render", at = @At("TAIL"))
	private void bench$skyFinished(CallbackInfo callback) {
		BenchMod.gpuEnd(GpuSpan.SKY, null);
	}
}
