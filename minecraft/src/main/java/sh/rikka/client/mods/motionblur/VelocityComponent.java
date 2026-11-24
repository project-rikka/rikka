package sh.rikka.client.mods.motionblur;

import com.mojang.renderpearl.api.pipeline.RenderPipeline;

enum VelocityComponent {
	TRANSLATION(MotionBlurPipelines.VELOCITY_TRANSLATION),
	ROTATION(MotionBlurPipelines.VELOCITY_ROTATION),
	FULL(MotionBlurPipelines.VELOCITY_FULL);

	private final RenderPipeline pipeline;

	VelocityComponent(RenderPipeline pipeline) {
		this.pipeline = pipeline;
	}

	RenderPipeline pipeline() {
		return this.pipeline;
	}
}
