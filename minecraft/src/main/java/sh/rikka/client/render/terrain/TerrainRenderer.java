package sh.rikka.client.render.terrain;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import com.mojang.renderpearl.api.pipeline.PrimitiveTopology;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import com.mojang.renderpearl.backend.vulkan.VulkanDevice;
import it.unimi.dsi.fastutil.objects.ObjectArrayList;
import java.util.Objects;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.client.renderer.fog.FogData;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.data.AtlasIds;
import net.minecraft.util.Util;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;
import org.joml.Matrix4fc;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mixin.terrain.FogRendererAccessor;
import sh.rikka.client.mods.boost.BoostFeature;
import sh.rikka.client.render.vulkan.ComputePipeline;
import sh.rikka.client.render.vulkan.VulkanCommands;

public final class TerrainRenderer implements AutoCloseable {
	private static final String CULL_SHADER = "cull.comp";
	private static volatile @Nullable TerrainRenderer instance;

	static {
		BoostFeature.LEAF_CULLING.onChange(TerrainRenderer::rebuildSections);
		BoostFeature.COMPACT_VERTICES.onChange(TerrainRenderer::rebuildSections);
	}

	private final VulkanDevice device;
	private final SectionTable table;
	private final ComputePipeline cull;
	private final TerrainView mainView;
	private final Matrix4f projection = new Matrix4f();
	private int visibleVersion;

	private TerrainRenderer(VulkanDevice device) {
		this.device = device;
		this.table = new SectionTable(device);
		this.cull = new ComputePipeline(device, TerrainRenderer.class, CULL_SHADER, TerrainView.CULL_DEFINES, TerrainView.CULL_DESCRIPTORS, TerrainView.CULL_CONSTANT_BYTES);
		this.mainView = new TerrainView(device);
	}

	public static TerrainRenderer get() {
		TerrainRenderer current = instance;
		if (current == null) {
			current = new TerrainRenderer(VulkanCommands.device());
			instance = current;
		}
		return current;
	}

	private static void rebuildSections() {
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft != null) {
			minecraft.schedule(() -> minecraft.levelExtractor.allChanged());
		}
	}

	public static void release() {
		TerrainRenderer current = instance;
		if (current != null) {
			instance = null;
			current.close();
		}
	}

	public static void sectionCreated(SectionRenderDispatcher.RenderSection section) {
		TerrainRenderer current = instance;
		if (current != null) {
			current.table.register(section);
		}
	}

	public static void sectionChanged(SectionRenderDispatcher.RenderSection section) {
		TerrainRenderer current = instance;
		if (current != null) {
			current.table.mark(section);
		}
	}

	public static void visibleSectionsChanged() {
		TerrainRenderer current = instance;
		if (current != null) {
			current.visibleVersion++;
		}
	}

	public static void renderProjection(Matrix4fc projection) {
		TerrainRenderer current = instance;
		if (current != null) {
			current.projection.set(projection);
		}
	}

	public void geometryInvalidated() {
		this.table.beginEpoch();
		this.visibleVersion++;
	}

	public TerrainView mainView() {
		return this.mainView;
	}

	public TerrainView createView() {
		return new TerrainView(this.device);
	}

	public void prepareMain(LevelRenderer level, Matrix4fc viewRotation) {
		CameraRenderState camera = Minecraft.getInstance().gameRenderer.gameRenderState().levelRenderState.cameraRenderState;
		this.prepare(this.mainView, level, level.visibleSections(), this.visibleVersion, this.projection, viewRotation, camera.pos, camera.fogData);
	}

	public void prepare(
		TerrainView view,
		LevelRenderer level,
		ObjectArrayList<SectionRenderDispatcher.RenderSection> visible,
		int visibleVersion,
		Matrix4fc projection,
		Matrix4fc viewRotation,
		Vec3 camera,
		FogData fog
	) {
		Minecraft minecraft = Minecraft.getInstance();
		SectionRenderDispatcher dispatcher = Objects.requireNonNull(level.sectionRenderDispatcher());
		long fadeMillis = Util.toMillis(minecraft.gameRenderer.gameRenderState().optionsRenderState.chunkSectionFadeInTime);
		this.table.flush(dispatcher, Util.getMillis(), fadeMillis);
		GpuTextureView atlas = minecraft.getAtlasManager().getAtlasOrThrow(AtlasIds.BLOCKS).getTextureView();
		GpuBufferSlice transform = RenderSystem.getDynamicUniforms().writeTerrainTransform(viewRotation, atlas.getWidth(0), atlas.getHeight(0));
		int largest = this.table.largestSequentialIndexCount();
		if (largest != 0) {
			RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS).requestIndexCount(largest);
		}
		view.prepare(this.table, this.cull, visible, visibleVersion, projection, viewRotation, camera, opaqueFogDistance(fog), transform);
	}

	private static float opaqueFogDistance(FogData fog) {
		return BoostFeature.FOG_CULLING.isActive() && FogRendererAccessor.rikka$fogEnabled() ? fog.renderDistanceEnd : Float.POSITIVE_INFINITY;
	}

	@Override
	public void close() {
		this.mainView.close();
		this.table.close();
		this.cull.close();
	}
}
