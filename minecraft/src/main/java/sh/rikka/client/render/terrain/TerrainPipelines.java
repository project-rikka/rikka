package sh.rikka.client.render.terrain;

import com.mojang.renderpearl.api.pipeline.ColorTargetState;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import com.mojang.renderpearl.api.pipeline.ShaderType;
import com.mojang.renderpearl.api.vertex.VertexFormat;
import java.util.EnumMap;
import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import net.minecraft.resources.Identifier;
import sh.rikka.client.RikkaClient;

final class TerrainPipelines {
	private static final Identifier COMPACT_VERTEX_SHADER = Identifier.fromNamespaceAndPath(RikkaClient.MOD_ID, "core/compact_terrain");
	private static final Identifier FAST_FRAGMENT_SHADER = Identifier.fromNamespaceAndPath(RikkaClient.MOD_ID, "core/terrain");
	private static final int TERRAIN_VERTEX_BINDING = 0;
	private static final Map<Variant, RenderPipeline> DERIVED = new HashMap<>();

	private TerrainPipelines() {
	}

	static RenderPipeline of(RenderPipeline vanilla, boolean compact, boolean fastFiltering) {
		if (!compact && !fastFiltering) {
			return vanilla;
		}
		return DERIVED.computeIfAbsent(new Variant(vanilla, compact, fastFiltering), TerrainPipelines::derive);
	}

	private static RenderPipeline derive(Variant variant) {
		RenderPipeline vanilla = variant.vanilla();
		Map<ShaderType, Identifier> shaders = new EnumMap<>(vanilla.getShaders());
		VertexFormat[] formats = vanilla.getVertexFormatBindings().toArray(VertexFormat[]::new);
		StringBuilder location = new StringBuilder("terrain/");
		if (variant.compact()) {
			shaders.put(ShaderType.VERTEX, COMPACT_VERTEX_SHADER);
			formats[TERRAIN_VERTEX_BINDING] = CompactVertices.FORMAT;
			location.append("compact_");
		}
		if (variant.fastFiltering()) {
			shaders.put(ShaderType.FRAGMENT, FAST_FRAGMENT_SHADER);
			location.append("fast_");
		}
		ColorTargetState[] targets = vanilla.getColorTargetStates().toArray(ColorTargetState[]::new);
		RenderPipeline.Snippet snippet = new RenderPipeline.Snippet(
			shaders,
			Optional.of(vanilla.getShaderDefines()),
			Optional.of(vanilla.getBindGroupLayouts()),
			targets,
			targets.length,
			Optional.ofNullable(vanilla.getDepthStencilState()),
			Optional.of(vanilla.getPolygonMode()),
			Optional.of(vanilla.isCull()),
			formats,
			Optional.of(vanilla.getPrimitiveTopology()),
			vanilla.pushConstantSize()
		);
		location.append(vanilla.getLocation().getPath().replace('/', '_'));
		return RenderPipeline.builder(snippet).withLocation(Identifier.fromNamespaceAndPath(RikkaClient.MOD_ID, location.toString())).build();
	}

	private record Variant(RenderPipeline vanilla, boolean compact, boolean fastFiltering) {
	}
}
