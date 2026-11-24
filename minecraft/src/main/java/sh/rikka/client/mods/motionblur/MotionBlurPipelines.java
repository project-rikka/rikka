package sh.rikka.client.mods.motionblur;

import com.mojang.renderpearl.api.pipeline.BindGroupLayout;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import com.mojang.renderpearl.api.pipeline.UniformType;
import sh.rikka.client.render.post.ScreenPipeline;

final class MotionBlurPipelines {
	static final int MAX_BLENDED_FRAMES = 24;
	static final String MAIN_DEPTH_SAMPLER = "MainDepthSampler";
	static final String HISTORY_SAMPLER = "HistorySampler";

	private static final String[] FRAME_SAMPLERS = frameSamplers();

	static final RenderPipeline VELOCITY_TRANSLATION = velocity("translation", "MOTION_BLUR_TRANSLATION");
	static final RenderPipeline VELOCITY_ROTATION = velocity("rotation", "MOTION_BLUR_ROTATION");
	static final RenderPipeline VELOCITY_FULL = velocity("full", "MOTION_BLUR_FULL");
	static final RenderPipeline FRAME_BLEND = frameBlend();
	static final RenderPipeline ACCUMULATE_MAX = accumulation("max", "MOTION_BLUR_ACCUMULATE_MAX");
	static final RenderPipeline ACCUMULATE_MIX = accumulation("mix", "MOTION_BLUR_ACCUMULATE_MIX");

	private MotionBlurPipelines() {
	}

	static String frameSampler(int index) {
		return FRAME_SAMPLERS[index];
	}

	private static RenderPipeline velocity(String variant, String define) {
		BindGroupLayout layout = BindGroupLayout.builder()
			.withUniform(ScreenPipeline.MAIN_SAMPLER, UniformType.COMBINED_IMAGE_SAMPLER)
			.withUniform(MAIN_DEPTH_SAMPLER, UniformType.COMBINED_IMAGE_SAMPLER)
			.withUniform(VelocityUniforms.BLOCK, UniformType.UNIFORM_BUFFER)
			.build();
		return screen("velocity_" + variant, "velocity", layout).withShaderDefine(define).build();
	}

	private static RenderPipeline frameBlend() {
		BindGroupLayout.Builder layout = BindGroupLayout.builder();
		for (String sampler : FRAME_SAMPLERS) {
			layout.withUniform(sampler, UniformType.COMBINED_IMAGE_SAMPLER);
		}
		layout.withUniform(FrameBlendUniforms.BLOCK, UniformType.UNIFORM_BUFFER);
		return screen("frame_blend", "frame_blend", layout.build()).build();
	}

	private static RenderPipeline accumulation(String variant, String define) {
		BindGroupLayout layout = BindGroupLayout.builder()
			.withUniform(ScreenPipeline.MAIN_SAMPLER, UniformType.COMBINED_IMAGE_SAMPLER)
			.withUniform(HISTORY_SAMPLER, UniformType.COMBINED_IMAGE_SAMPLER)
			.withUniform(AccumulationUniforms.BLOCK, UniformType.UNIFORM_BUFFER)
			.build();
		return screen("accumulate_" + variant, "accumulation", layout).withShaderDefine(define).build();
	}

	private static RenderPipeline.Builder screen(String name, String shader, BindGroupLayout layout) {
		return ScreenPipeline.builder("motion_blur/" + name, "motion_blur/" + shader, layout);
	}

	private static String[] frameSamplers() {
		String[] samplers = new String[MAX_BLENDED_FRAMES];
		for (int i = 0; i < MAX_BLENDED_FRAMES; i++) {
			samplers[i] = "Sample" + i + "Sampler";
		}
		return samplers;
	}
}
