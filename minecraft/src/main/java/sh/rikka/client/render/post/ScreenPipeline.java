package sh.rikka.client.render.post;

import com.mojang.renderpearl.api.pipeline.BindGroupLayout;
import com.mojang.renderpearl.api.pipeline.ColorTargetState;
import com.mojang.renderpearl.api.pipeline.PrimitiveTopology;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import net.minecraft.resources.Identifier;
import sh.rikka.client.RikkaClient;

public final class ScreenPipeline {
	public static final String MAIN_SAMPLER = "MainSampler";

	private static final String SCREEN_QUAD = "core/screenquad";
	private static final String PIPELINE_FOLDER = "pipeline/";
	private static final String SHADER_FOLDER = "post/";

	private ScreenPipeline() {
	}

	public static RenderPipeline.Builder builder(String location, String fragment, BindGroupLayout layout) {
		return RenderPipeline.builder()
			.withLocation(id(PIPELINE_FOLDER + location))
			.withVertexShader(SCREEN_QUAD)
			.withFragmentShader(id(SHADER_FOLDER + fragment))
			.withPrimitiveTopology(PrimitiveTopology.TRIANGLES)
			.withBindGroupLayout(layout)
			.withColorTargetState(ColorTargetState.DEFAULT);
	}

	private static Identifier id(String path) {
		return Identifier.fromNamespaceAndPath(RikkaClient.MOD_ID, path);
	}
}
