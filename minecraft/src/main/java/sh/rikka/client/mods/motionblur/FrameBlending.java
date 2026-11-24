package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;

final class FrameBlending {
	private static final int MAX_FRAMES = MotionBlurPipelines.MAX_BLENDED_FRAMES;
	private static final int SPARE_FRAMES = 2;
	private static final int SHRINK_DELAY_FRAMES = 120;

	private final FrameHistory history = new FrameHistory();
	private final ExposureWeights weights = new ExposureWeights();
	private final UniformStream<FrameBlendUniforms> uniforms = new UniformStream<>("Motion blur frame blend", FrameBlendUniforms.SIZE);
	private int capacity = SPARE_FRAMES;
	private int framesBelowCapacity;

	void apply(float strength, float fps, float refreshHz, double now, RenderTarget main) {
		double exposure = strength / (fps > 0.0F ? Math.min(fps, refreshHz) : refreshHz);
		double frameTime = 1.0 / (fps > 0.0F ? fps : refreshHz);
		this.history.record(main, now, exposure / (MAX_FRAMES - SPARE_FRAMES), capacity(exposure / frameTime));
		int count = this.weights.compute(this.history, now, exposure, frameTime);
		if (count <= 1) {
			return;
		}
		GpuBufferSlice block = this.uniforms.write(this.weights.uniforms());
		RenderTarget newest = this.history.target(this.weights.frame(count - 1));
		ScreenPass.draw(MotionBlurPipelines.FRAME_BLEND, main, pass -> {
			for (int i = 0; i < MAX_FRAMES; i++) {
				RenderTarget frame = i < count ? this.history.target(this.weights.frame(i)) : newest;
				pass.setUniform(MotionBlurPipelines.frameSampler(i), frame.getColorTextureView(), ScreenPass.nearest());
			}
			pass.setUniform(FrameBlendUniforms.BLOCK, block);
		});
	}

	void restart() {
		this.history.clear();
	}

	void endFrame() {
		this.uniforms.endFrame();
	}

	void close() {
		this.history.clear();
		this.uniforms.close();
		this.capacity = SPARE_FRAMES;
		this.framesBelowCapacity = 0;
	}

	private int capacity(double exposedFrames) {
		int wanted = Math.clamp((long) Math.ceil(exposedFrames) + SPARE_FRAMES, SPARE_FRAMES, MAX_FRAMES);
		if (wanted >= this.capacity) {
			this.capacity = wanted;
			this.framesBelowCapacity = 0;
		} else if (++this.framesBelowCapacity >= SHRINK_DELAY_FRAMES) {
			this.capacity = wanted;
			this.framesBelowCapacity = 0;
		}
		return this.capacity;
	}
}
