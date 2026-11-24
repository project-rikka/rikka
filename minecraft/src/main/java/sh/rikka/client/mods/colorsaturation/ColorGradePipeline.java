package sh.rikka.client.mods.colorsaturation;

import com.mojang.renderpearl.api.pipeline.BindGroupLayout;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import com.mojang.renderpearl.api.pipeline.UniformType;
import sh.rikka.client.render.post.ScreenPipeline;

final class ColorGradePipeline {
	static final RenderPipeline GRADE = ScreenPipeline.builder(
		"color_saturation/grade",
		"color_saturation/grade",
		BindGroupLayout.builder()
			.withUniform(ScreenPipeline.MAIN_SAMPLER, UniformType.COMBINED_IMAGE_SAMPLER)
			.withUniform(ColorGrade.BLOCK, UniformType.UNIFORM_BUFFER)
			.build()
	).build();

	private ColorGradePipeline() {
	}
}
