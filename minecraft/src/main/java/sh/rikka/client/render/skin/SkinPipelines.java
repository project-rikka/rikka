package sh.rikka.client.render.skin;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.renderpearl.api.pipeline.BindGroupLayout;
import com.mojang.renderpearl.api.pipeline.ColorTargetState;
import com.mojang.renderpearl.api.pipeline.PrimitiveTopology;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import com.mojang.renderpearl.api.pipeline.ShaderType;
import com.mojang.renderpearl.api.vertex.VertexFormat;
import it.unimi.dsi.fastutil.objects.Reference2ReferenceOpenHashMap;
import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.resources.Resource;
import net.minecraft.server.packs.resources.ResourceManager;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.RikkaClient;

public final class SkinPipelines {
	private static final String VANILLA_PACK = "vanilla";
	private static final Map<Identifier, Shader> SHADERS = Map.of(
		Identifier.withDefaultNamespace("core/entity"), new Shader("core/skin_entity", DefaultVertexFormat.ENTITY),
		Identifier.withDefaultNamespace("core/item"), new Shader("core/skin_item", DefaultVertexFormat.ENTITY),
		Identifier.withDefaultNamespace("core/glint"), new Shader("core/skin_glint", DefaultVertexFormat.POSITION_TEX)
	);

	private final Map<SkinFaces, Reference2ReferenceOpenHashMap<RenderPipeline, Optional<RenderPipeline>>> twins = new EnumMap<>(SkinFaces.class);
	private boolean available;
	private @Nullable RenderPipeline lastChecked;
	private boolean lastSupported;

	public SkinPipelines() {
		for (SkinFaces faces : SkinFaces.values()) {
			this.twins.put(faces, new Reference2ReferenceOpenHashMap<>());
		}
	}

	public void reload(ResourceManager resources) {
		this.available = SHADERS.keySet().stream().allMatch(shader -> isVanilla(resources, shader));
		this.lastChecked = null;
	}

	public boolean compileAll() {
		List<RenderPipeline> pipelines = new ArrayList<>(RenderPipelines.requiredPipelines());
		pipelines.addAll(RenderPipelines.optionalPipelines());
		for (RenderPipeline pipeline : pipelines) {
			for (SkinFaces faces : SkinFaces.values()) {
				RenderPipeline twin = faces.appliesTo(pipeline) ? twin(pipeline, faces) : null;
				if (twin != null && RenderSystem.getCompiledPipelineNullable(twin) == null) {
					RikkaClient.LOGGER.error("failed to compile {}, drawing entities without skin pipelines", twin.getLocation());
					this.available = false;
					this.lastChecked = null;
					return false;
				}
			}
		}
		return true;
	}

	public boolean supports(RenderPipeline pipeline) {
		if (pipeline != this.lastChecked) {
			this.lastSupported = twin(pipeline, SkinFaces.ALL) != null;
			this.lastChecked = pipeline;
		}
		return this.lastSupported;
	}

	public @Nullable RenderPipeline twin(RenderPipeline pipeline, SkinFaces faces) {
		if (!this.available) {
			return null;
		}
		Reference2ReferenceOpenHashMap<RenderPipeline, Optional<RenderPipeline>> twins = this.twins.get(faces);
		Optional<RenderPipeline> twin = twins.get(pipeline);
		if (twin == null) {
			twin = Optional.ofNullable(create(pipeline, faces));
			twins.put(pipeline, twin);
		}
		return twin.orElse(null);
	}

	private static boolean isVanilla(ResourceManager resources, Identifier shader) {
		Identifier file = ShaderType.VERTEX.idConverter().idToFile(shader);
		return resources.getResource(file).map(Resource::sourcePackId).filter(VANILLA_PACK::equals).isPresent();
	}

	private static @Nullable RenderPipeline create(RenderPipeline pipeline, SkinFaces faces) {
		Shader shader = SHADERS.get(pipeline.getShaders().get(ShaderType.VERTEX));
		if (shader == null || pipeline.getPrimitiveTopology() != PrimitiveTopology.QUADS || !shader.replaces(pipeline)) {
			return null;
		}
		return new Twin(pipeline, shader.vertexShader(), faces);
	}

	private record Shader(Identifier vertexShader, VertexFormat vertices) {
		private Shader(String vertexShader, VertexFormat vertices) {
			this(Identifier.fromNamespaceAndPath(RikkaClient.MOD_ID, vertexShader), vertices);
		}

		private boolean replaces(RenderPipeline pipeline) {
			List<@Nullable VertexFormat> bindings = pipeline.getVertexFormatBindings();
			for (int binding = 1; binding < bindings.size(); binding++) {
				if (bindings.get(binding) != null) {
					return false;
				}
			}
			return this.vertices.equals(bindings.get(0));
		}
	}

	private static final class Twin extends RenderPipeline {
		private Twin(RenderPipeline source, Identifier vertexShader, SkinFaces faces) {
			super(
				Identifier.fromNamespaceAndPath(RikkaClient.MOD_ID, "pipeline/skin/" + faces.pathSegment() + source.getLocation().getNamespace() + "/" + source.getLocation().getPath()),
				shaders(source, vertexShader),
				source.getShaderDefines().withOverrides(faces.defines()),
				layouts(source),
				source.getColorTargetStates().toArray(ColorTargetState[]::new),
				source.getDepthStencilState(),
				source.getPolygonMode(),
				source.isCull(),
				bindings(source),
				source.getPrimitiveTopology(),
				source.pushConstantSize(),
				source.getSortKey()
			);
		}

		private static Map<ShaderType, Identifier> shaders(RenderPipeline source, Identifier vertexShader) {
			Map<ShaderType, Identifier> shaders = new EnumMap<>(source.getShaders());
			shaders.put(ShaderType.VERTEX, vertexShader);
			return shaders;
		}

		private static List<BindGroupLayout> layouts(RenderPipeline source) {
			List<BindGroupLayout> layouts = new ArrayList<>(source.getBindGroupLayouts());
			layouts.add(SkinLayout.PALETTE);
			return layouts;
		}

		private static @Nullable VertexFormat[] bindings(RenderPipeline source) {
			@Nullable VertexFormat[] bindings = new VertexFormat[source.getVertexFormatBindings().size()];
			bindings[0] = SkinLayout.VERTEX;
			bindings[1] = SkinLayout.INSTANCE;
			return bindings;
		}
	}
}
