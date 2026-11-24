package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;

final class VelocityBlur {
	private final UniformStream<VelocityUniforms> uniforms = new UniformStream<>("Motion blur velocity", VelocityUniforms.SIZE);
	private final ScreenBuffer scratch = new ScreenBuffer("Motion blur velocity output");

	void apply(VelocityComponent component, VelocityUniforms data, RenderTarget main) {
		GpuBufferSlice block = this.uniforms.write(data);
		RenderTarget output = this.scratch.sizedLike(main);
		ScreenPass.draw(component.pipeline(), output, pass -> {
			pass.setUniform(MotionBlurPipelines.MAIN_SAMPLER, main.getColorTextureView(), ScreenPass.linear());
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
