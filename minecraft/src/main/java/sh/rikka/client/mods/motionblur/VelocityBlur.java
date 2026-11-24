package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import sh.rikka.client.render.post.ScreenBuffer;
import sh.rikka.client.render.post.ScreenPass;
import sh.rikka.client.render.post.ScreenPipeline;
import sh.rikka.client.render.post.UniformStream;

final class VelocityBlur {
	private final UniformStream<VelocityUniforms> uniforms = new UniformStream<>("Motion blur velocity", VelocityUniforms.SIZE);
	private final ScreenBuffer scratch = new ScreenBuffer("Motion blur velocity output");

	void apply(VelocityComponent component, VelocityUniforms data, RenderTarget main) {
		GpuBufferSlice block = this.uniforms.write(data);
		RenderTarget output = this.scratch.sizedLike(main);
		ScreenPass.draw(component.pipeline(), output, pass -> {
			pass.setUniform(ScreenPipeline.MAIN_SAMPLER, main.getColorTextureView(), ScreenPass.linear());
			pass.setUniform(MotionBlurPipelines.MAIN_DEPTH_SAMPLER, main.getDepthTextureView(), ScreenPass.nearest());
			pass.setUniform(VelocityUniforms.BLOCK, block);
		});
		ScreenPass.copy(output, main);
	}

	void endFrame() {
		this.uniforms.endFrame();
	}

	void close() {
		this.uniforms.close();
		this.scratch.close();
	}
}
