package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.commands.RenderPass;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import com.mojang.renderpearl.api.textures.FilterMode;
import com.mojang.renderpearl.api.textures.GpuSampler;
import java.util.Optional;
import java.util.function.Consumer;

final class ScreenPass {
	private ScreenPass() {
	}

	static void draw(RenderPipeline pipeline, RenderTarget output, Consumer<RenderPass> bindings) {
		try (RenderPass pass = RenderSystem.getDevice()
			.createCommandEncoder()
			.createRenderPass(() -> "Motion blur " + pipeline.getLocation(), output.getColorTextureView(), Optional.empty())) {
			pass.setPipeline(RenderSystem.getCompiledPipeline(pipeline));
			bindings.accept(pass);
			pass.draw(3, 1, 0, 0);
		}
	}

	static void copy(RenderTarget source, RenderTarget destination) {
		RenderSystem.getDevice()
			.createCommandEncoder()
			.copyTextureToTexture(source.getColorTexture(), destination.getColorTexture(), 0, 0, 0, 0, 0, destination.width, destination.height);
	}

	static GpuSampler nearest() {
		return RenderSystem.getSamplerCache().getClampToEdge(FilterMode.NEAREST);
	}

	static GpuSampler linear() {
		return RenderSystem.getSamplerCache().getClampToEdge(FilterMode.LINEAR);
	}
}
