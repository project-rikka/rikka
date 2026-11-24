package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;

final class Accumulation {
	private static final float MAX_STRENGTH_SCALE = 7.0F;
	private static final float MIX_STRENGTH_SCALE = 5.0F;
	private static final double HALF_LIFE_STRENGTH = 3.0;

	private final UniformStream<AccumulationUniforms> uniforms = new UniformStream<>("Motion blur accumulation", AccumulationUniforms.SIZE);
	private ScreenBuffer history = new ScreenBuffer("Motion blur accumulation history");
	private ScreenBuffer output = new ScreenBuffer("Motion blur accumulation output");
	private boolean primed;

	void apply(BlurAlgorithm algorithm, float strength, RenderTarget main) {
		boolean continuing = this.primed && this.history.fits(main);
		RenderTarget previous = this.history.sizedLike(main);
		if (!continuing) {
			ScreenPass.copy(main, previous);
			this.primed = true;
			return;
		}
		RenderPipeline pipeline = algorithm == BlurAlgorithm.ACCUMULATION_MAX ? MotionBlurPipelines.ACCUMULATE_MAX : MotionBlurPipelines.ACCUMULATE_MIX;
		float scale = algorithm == BlurAlgorithm.ACCUMULATION_MAX ? MAX_STRENGTH_SCALE : MIX_STRENGTH_SCALE;
		GpuBufferSlice block = this.uniforms.write(new AccumulationUniforms(blendFactor(strength * scale)));
		RenderTarget blended = this.output.sizedLike(main);
		ScreenPass.draw(pipeline, blended, pass -> {
			pass.setUniform(MotionBlurPipelines.MAIN_SAMPLER, main.getColorTextureView(), ScreenPass.nearest());
			pass.setUniform(MotionBlurPipelines.HISTORY_SAMPLER, previous.getColorTextureView(), ScreenPass.nearest());
			pass.setUniform(AccumulationUniforms.BLOCK, block);
		});
		ScreenPass.copy(blended, main);
		ScreenBuffer swapped = this.history;
		this.history = this.output;
		this.output = swapped;
	}

	void restart() {
		this.primed = false;
	}

	void endFrame() {
		this.uniforms.endFrame();
	}

	void close() {
		this.history.close();
		this.output.close();
		this.uniforms.close();
		this.primed = false;
	}

	private static float blendFactor(float strength) {
		return (float) (1.0 - Math.pow(0.5, strength / HALF_LIFE_STRENGTH));
	}
}
