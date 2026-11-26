package sh.rikka.bench.mixin;

import com.mojang.renderpearl.api.commands.RenderPass;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.chunk.ChunkSectionsToRender;
import net.minecraft.client.renderer.feature.FeatureRenderDispatcher;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.bench.BenchMod;
import sh.rikka.bench.GpuSpan;

@Mixin(LevelRenderer.class)
abstract class LevelRendererMixin {
	@Inject(method = "executeSolid", at = @At("HEAD"))
	private void bench$solidStarting(ChunkSectionsToRender sections, FeatureRenderDispatcher.PreparedFrame frame, RenderPass pass, CallbackInfo callback) {
		BenchMod.gpuBegin(GpuSpan.SOLID, pass);
	}

	@Inject(method = "executeClassicTransparency", at = @At("HEAD"))
	private void bench$transparentStarting(
		ChunkSectionsToRender sections,
		FeatureRenderDispatcher.PreparedFrame frame,
		RenderPass pass,
		CallbackInfo callback
	) {
		BenchMod.gpuEnd(GpuSpan.SOLID, pass);
		BenchMod.gpuBegin(GpuSpan.TRANSPARENT, pass);
	}

	@Inject(method = "executeClassicTransparency", at = @At("TAIL"))
	private void bench$transparentFinished(
		ChunkSectionsToRender sections,
		FeatureRenderDispatcher.PreparedFrame frame,
		RenderPass pass,
		CallbackInfo callback
	) {
		BenchMod.gpuEnd(GpuSpan.TRANSPARENT, pass);
	}
}
