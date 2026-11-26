package sh.rikka.client.mods.boost.scale;

import com.mojang.renderpearl.api.pipeline.BindGroupLayout;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import com.mojang.renderpearl.api.pipeline.UniformType;
import sh.rikka.client.render.post.ScreenPipeline;

final class UpscalePipelines {
	static final RenderPipeline UPSAMPLE = ScreenPipeline.builder(
		"upscale/upsample",
		"upscale/upsample",
		BindGroupLayout.builder().withUniform(ScreenPipeline.MAIN_SAMPLER, UniformType.COMBINED_IMAGE_SAMPLER).build()
	).build();

	static final RenderPipeline SHARPEN = ScreenPipeline.builder(
		"upscale/sharpen",
		"upscale/sharpen",
		BindGroupLayout.builder()
			.withUniform(ScreenPipeline.MAIN_SAMPLER, UniformType.COMBINED_IMAGE_SAMPLER)
			.withUniform(Sharpening.BLOCK, UniformType.UNIFORM_BUFFER)
			.build()
	).build();

	private UpscalePipelines() {
	}
}
