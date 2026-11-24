package sh.rikka.client.mods.motionblur;

import java.util.Arrays;

final class ExposureWeights {
	private static final double MIN_OVERLAP = 1.0e-7;

	private final int[] frames = new int[MotionBlurPipelines.MAX_BLENDED_FRAMES];
	private final float[] weights = new float[MotionBlurPipelines.MAX_BLENDED_FRAMES];
	private int count;
	private double totalWeight;

	int compute(FrameHistory history, double exposureEnd, double exposure, double frameTime) {
		double exposureStart = exposureEnd - exposure;
		this.count = 0;
		this.totalWeight = 0.0;
		for (int i = 0; i < history.size(); i++) {
			double frameEnd = history.time(i);
			double frameStart = i > 0 ? history.time(i - 1) : frameEnd - frameTime;
			if (frameStart >= frameEnd) {
				frameStart = frameEnd - frameTime;
			}
			double overlap = Math.min(frameEnd, exposureEnd) - Math.max(frameStart, exposureStart);
			if (overlap > MIN_OVERLAP) {
				this.frames[this.count] = i;
				this.weights[this.count] = (float) overlap;
				this.totalWeight += overlap;
				this.count++;
			}
		}
		if (this.totalWeight <= MIN_OVERLAP) {
			this.count = 0;
		}
		return this.count;
	}

	int frame(int index) {
		return this.frames[index];
	}

	FrameBlendUniforms uniforms() {
		return new FrameBlendUniforms((float) (1.0 / this.totalWeight), Arrays.copyOf(this.weights, this.count));
	}
}
