package sh.rikka.client.mods.colorsaturation;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import sh.rikka.client.render.post.ScreenBuffer;
import sh.rikka.client.render.post.ScreenPass;
import sh.rikka.client.render.post.ScreenPipeline;
import sh.rikka.client.render.post.UniformStream;

final class ColorGradePass {
	private final UniformStream<ColorGrade> uniforms = new UniformStream<>("Color grade", ColorGrade.SIZE);
	private final ScreenBuffer output = new ScreenBuffer("Color grade output");

	void apply(ColorGrade grade, RenderTarget main) {
		GpuBufferSlice block = this.uniforms.write(grade);
		RenderTarget graded = this.output.sizedLike(main);
		ScreenPass.draw(ColorGradePipeline.GRADE, graded, pass -> {
			pass.setUniform(ScreenPipeline.MAIN_SAMPLER, main.getColorTextureView(), ScreenPass.nearest());
			pass.setUniform(ColorGrade.BLOCK, block);
		});
		ScreenPass.copy(graded, main);
		this.uniforms.endFrame();
	}

	void close() {
		this.output.close();
		this.uniforms.close();
	}
}
