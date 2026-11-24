package sh.rikka.client.render.post;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.commands.RenderPass;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import com.mojang.renderpearl.api.textures.FilterMode;
import com.mojang.renderpearl.api.textures.GpuSampler;
import java.util.Optional;
import java.util.function.Consumer;

public final class ScreenPass {
	private ScreenPass() {
	}

	public static void draw(RenderPipeline pipeline, RenderTarget output, Consumer<RenderPass> bindings) {
		try (RenderPass pass = RenderSystem.getDevice()
			.createCommandEncoder()
			.createRenderPass(() -> "Screen pass " + pipeline.getLocation(), output.getColorTextureView(), Optional.empty())) {
			pass.setPipeline(RenderSystem.getCompiledPipeline(pipeline));
			bindings.accept(pass);
			pass.draw(3, 1, 0, 0);
		}
	}

	public static void copy(RenderTarget source, RenderTarget destination) {
		RenderSystem.getDevice()
			.createCommandEncoder()
			.copyTextureToTexture(source.getColorTexture(), destination.getColorTexture(), 0, 0, 0, 0, 0, destination.width, destination.height);
	}

	public static GpuSampler nearest() {
		return RenderSystem.getSamplerCache().getClampToEdge(FilterMode.NEAREST);
	}

	public static GpuSampler linear() {
		return RenderSystem.getSamplerCache().getClampToEdge(FilterMode.LINEAR);
	}
}
