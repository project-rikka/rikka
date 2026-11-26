package sh.rikka.client.mods.boost.scale;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import sh.rikka.client.render.post.ScreenBuffer;
import sh.rikka.client.render.post.ScreenPass;
import sh.rikka.client.render.post.ScreenPipeline;
import sh.rikka.client.render.post.UniformStream;

final class Upscaler {
	private final ScreenBuffer upsampled = new ScreenBuffer("Upsampled world");
	private final UniformStream<Sharpening> uniforms = new UniformStream<>("Sharpening", Sharpening.SIZE);

	void apply(RenderTarget source, RenderTarget output, float sharpness) {
		if (sharpness <= 0.0F) {
			this.upsampled.close();
			upsample(source, output);
			return;
		}
		RenderTarget upsampled = this.upsampled.sizedLike(output);
		upsample(source, upsampled);
		GpuBufferSlice block = this.uniforms.write(new Sharpening(sharpness));
		ScreenPass.draw(UpscalePipelines.SHARPEN, output, pass -> {
			pass.setUniform(ScreenPipeline.MAIN_SAMPLER, upsampled.getColorTextureView(), ScreenPass.nearest());
			pass.setUniform(Sharpening.BLOCK, block);
		});
		this.uniforms.endFrame();
	}

	void close() {
		this.upsampled.close();
		this.uniforms.close();
	}

	private static void upsample(RenderTarget source, RenderTarget output) {
		ScreenPass.draw(
			UpscalePipelines.UPSAMPLE,
			output,
			pass -> pass.setUniform(ScreenPipeline.MAIN_SAMPLER, source.getColorTextureView(), ScreenPass.nearest())
		);
	}
}
