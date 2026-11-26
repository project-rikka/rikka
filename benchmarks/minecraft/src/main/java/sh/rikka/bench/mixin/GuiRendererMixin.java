package sh.rikka.bench.mixin;

import net.minecraft.client.gui.render.GuiRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.bench.BenchMod;
import sh.rikka.bench.GpuSpan;

@Mixin(GuiRenderer.class)
abstract class GuiRendererMixin {
	@Inject(method = "render", at = @At("HEAD"))
	private void bench$guiStarting(CallbackInfo callback) {
		BenchMod.gpuBegin(GpuSpan.GUI, null);
	}

	@Inject(method = "render", at = @At("TAIL"))
	private void bench$guiFinished(CallbackInfo callback) {
		BenchMod.gpuEnd(GpuSpan.GUI, null);
	}
}
