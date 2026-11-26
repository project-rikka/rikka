package sh.rikka.client.mods.rearview;

import com.mojang.blaze3d.ProjectionType;
import com.mojang.blaze3d.pipeline.TextureTarget;
import com.mojang.blaze3d.platform.Lighting;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.GpuFormat;
import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import com.mojang.renderpearl.api.commands.CommandEncoder;
import com.mojang.renderpearl.api.commands.RenderPass;
import com.mojang.renderpearl.api.commands.RenderPassDescriptor;
import com.mojang.renderpearl.api.textures.GpuSampler;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import it.unimi.dsi.fastutil.objects.ObjectArrayList;
import java.util.Objects;
import java.util.Optional;
import java.util.OptionalDouble;
import net.minecraft.client.CloudStatus;
import net.minecraft.client.Minecraft;
import net.minecraft.client.TextureFilteringMethod;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.GlobalSettingsUniform;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.ProjectionMatrixBuffer;
import net.minecraft.client.renderer.SkyRenderer;
import net.minecraft.client.renderer.SubmitNodeCollection;
import net.minecraft.client.renderer.SubmitNodeStorage;
import net.minecraft.client.renderer.chunk.ChunkSectionLayerGroup;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.feature.FeatureRenderDispatcher;
import net.minecraft.client.renderer.fog.FogData;
import net.minecraft.client.renderer.fog.FogRenderer;
import net.minecraft.client.renderer.state.GameRenderState;
import net.minecraft.client.renderer.state.OptionsRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.state.level.LevelRenderState;
import net.minecraft.core.BlockPos;
import net.minecraft.core.SectionPos;
import net.minecraft.data.AtlasIds;
import net.minecraft.util.ARGB;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.material.FogType;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;
import org.joml.Matrix4fStack;
import org.joml.Vector4f;
import org.joml.Vector4fc;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mixin.LevelExtractorInvoker;
import sh.rikka.client.render.terrain.TerrainRenderer;
import sh.rikka.client.render.terrain.TerrainView;

final class RearviewRenderer implements AutoCloseable {
	private static final String LABEL = "Rikka rearview";
	private static final String SEE_THROUGH_LABEL = "Rikka rearview see through features";
	private static final float NEAR_PLANE = 0.05F;
	private static final float CULL_MARGIN = 16.0F;
	private static final float PARTICLE_FRUSTUM_OFFSET = -3.0F;
	private static final float FOG_SPAN_FRACTION = 0.1F;
	private static final float MIN_FOG_SPAN = 4.0F;
	private static final float MAX_FOG_SPAN = 64.0F;
	private static final float BEHIND = 180.0F;
	private static final Vector4fc UNDRAWN = new Vector4f(0.0F, 0.0F, 0.0F, 1.0F);

	private final RearviewCamera camera = new RearviewCamera();
	private final CameraRenderState cameraState = new CameraRenderState();
	private final LevelRenderState levelState = new LevelRenderState();
	private final SubmitNodeStorage submits = new SubmitNodeStorage();
	private final ObjectArrayList<SectionRenderDispatcher.RenderSection> sections = new ObjectArrayList<>();
	private final ObjectArrayList<SectionRenderDispatcher.RenderSection> nearbySections = new ObjectArrayList<>();
	private final ObjectArrayList<SectionRenderDispatcher.RenderSection> builtSections = new ObjectArrayList<>();
	private final FogData fogData = new FogData();
	private final Matrix4f viewRotation = new Matrix4f();
	private final Matrix4f projection = new Matrix4f();
	private final FramePacer pacer = new FramePacer();
	private @Nullable TextureTarget target;
	private @Nullable SkyRenderer sky;
	private @Nullable FogRenderer fog;
	private @Nullable GlobalSettingsUniform globals;
	private @Nullable ProjectionMatrixBuffer projectionBuffer;
	private @Nullable Request pending;
	private @Nullable TerrainView terrain;
	private @Nullable TerrainRenderer terrainRenderer;
	private int sectionsVersion;

	RearviewRenderer() {
		this.levelState.cameraRenderState = this.cameraState;
		this.cameraState.fogData = this.fogData;
		((RearviewLevel) Minecraft.getInstance().levelRenderer).rikka$followSections(this.sections, this.nearbySections);
	}

	void detach() {
		((RearviewLevel) Minecraft.getInstance().levelRenderer).rikka$followSections(null, null);
	}

	GpuTextureView request(Request request) {
		TextureTarget target = this.target;
		if (target == null || target.width != request.width() || target.height != request.height()) {
			target = allocate(request.width(), request.height());
		}
		this.pending = request;
		return Objects.requireNonNull(target.getColorTextureView());
	}

	void reloadResources() {
		closeSky();
	}

	void render(MainView main) {
		Request request = this.pending;
		this.pending = null;
		TextureTarget target = this.target;
		Minecraft minecraft = Minecraft.getInstance();
		Entity entity = minecraft.getCameraEntity();
		GpuSampler terrainSampler = ((RearviewLevel) minecraft.levelRenderer).rikka$chunkLayerSampler();
		if (request == null || target == null || entity == null || terrainSampler == null || minecraft.levelRenderer.viewArea() == null) {
			return;
		}
		if (!this.pacer.due(request.frameRate())) {
			return;
		}
		draw(new Frame(request, main, target, entity, terrainSampler));
	}

	@Override
	public void close() {
		closeSky();
		closeTerrain();
		if (this.target != null) {
			this.target.destroyBuffers();
			this.target = null;
		}
		if (this.fog != null) {
			this.fog.close();
			this.fog = null;
		}
		if (this.globals != null) {
			this.globals.close();
			this.globals = null;
		}
		if (this.projectionBuffer != null) {
			this.projectionBuffer.close();
			this.projectionBuffer = null;
		}
	}

	private TextureTarget allocate(int width, int height) {
		TextureTarget target = this.target;
		if (target == null) {
			target = new TextureTarget(LABEL, width, height, GpuFormat.RGBA8_UNORM, GpuFormat.D32_FLOAT);
			this.target = target;
		} else {
			target.resize(width, height);
		}
		RenderSystem.getDevice()
			.createCommandEncoder()
			.clearColorAndDepthTextures(target.getColorTexture(), UNDRAWN, target.getDepthTexture(), 0.0);
		this.pacer.invalidate();
		return target;
	}

	private void draw(Frame frame) {
		Minecraft minecraft = Minecraft.getInstance();
		GameRenderState gameState = minecraft.gameRenderer.gameRenderState();
		LevelRenderState mainLevel = gameState.levelRenderState;
		OptionsRenderState options = gameState.optionsRenderState;
		CameraRenderState mainCamera = frame.main().camera();
		float distance = Math.min(frame.request().distance(), options.renderDistance) * SectionPos.SECTION_SIZE;
		Frustum frustum = aim(frame, mainCamera, distance);
		shadeFog(mainCamera.fogData, distance);

		Matrix4fStack modelView = RenderSystem.getModelViewStack();
		GpuBufferSlice savedProjection = RenderSystem.getProjectionMatrixBuffer();
		ProjectionType savedProjectionType = RenderSystem.getProjectionType();
		GpuBufferSlice savedFog = RenderSystem.getShaderFog();
		GpuBufferSlice savedLights = RenderSystem.getShaderLights();
		GpuBuffer savedGlobals = RenderSystem.getGlobalSettingsUniform();
		boolean savedRenderingLevel = RenderSystem.isRenderingLevel;
		modelView.pushMatrix();
		try {
			extract(minecraft, mainLevel, frustum);
			modelView.mul(this.viewRotation);
			RenderSystem.isRenderingLevel = true;
			FogRenderer fog = fog();
			fog.updateBuffer(this.fogData);
			GpuBufferSlice fogBuffer = fog.getBuffer(FogRenderer.FogMode.WORLD);
			RenderSystem.setProjectionMatrix(projectionBuffer().getBuffer(this.projection), ProjectionType.PERSPECTIVE);
			globals().update(
				frame.target().width,
				frame.target().height,
				options.glintStrength,
				mainLevel.gameTime,
				mainLevel.worldPartialTicks,
				options.menuBackgroundBlurriness,
				this.cameraState.pos,
				options.textureFiltering == TextureFilteringMethod.RGSS
			);
			drawLevel(minecraft, frame, mainLevel, options, fogBuffer);
			fog.endFrame();
		} finally {
			this.levelState.particlesRenderState.reset();
			modelView.popMatrix();
			RenderSystem.isRenderingLevel = savedRenderingLevel;
			RenderSystem.setProjectionMatrix(savedProjection, savedProjectionType);
			RenderSystem.setShaderFog(savedFog);
			RenderSystem.setShaderLights(savedLights);
			RenderSystem.setGlobalSettingsUniform(savedGlobals);
		}
	}

	private Frustum aim(Frame frame, CameraRenderState main, float distance) {
		Entity entity = frame.entity();
		float partialTicks = main.cameraEntityPartialTicks;
		Vec3 position = main.isFirstPerson ? main.pos : entity.getEyePosition(partialTicks);
		float yRot = entity.getViewYRot(partialTicks) + BEHIND;
		float xRot = frame.request().followPitch() ? entity.getViewXRot(partialTicks) : 0.0F;
		this.camera.aim(entity, position, yRot, xRot);

		float aspect = (float) frame.target().width / frame.target().height;
		float fov = frame.request().fov() * Mth.DEG_TO_RAD;
		boolean zeroToOne = RenderSystem.getDevice().getDeviceInfo().isZZeroToOne();
		this.camera.getViewRotationMatrix(this.viewRotation);
		this.projection.setPerspective(fov, aspect, main.depthFar, NEAR_PLANE, zeroToOne);
		Matrix4f cullProjection = new Matrix4f().perspective(fov, aspect, NEAR_PLANE, distance + CULL_MARGIN, zeroToOne);
		Frustum frustum = new Frustum(this.viewRotation, cullProjection);
		frustum.prepare(position.x, position.y, position.z);

		CameraRenderState state = this.cameraState;
		state.blockPos = BlockPos.containing(position);
		state.pos = position;
		state.xRot = xRot;
		state.yRot = yRot;
		state.initialized = true;
		state.isPanoramicMode = false;
		state.isFrustumCaptured = false;
		state.isFirstPerson = true;
		state.smartCull = main.smartCull;
		state.orientation.set(this.camera.rotation());
		state.cameraEntityPartialTicks = partialTicks;
		state.cullFrustum.set(frustum);
		state.fogType = main.fogType;
		state.hudFov = main.hudFov;
		state.depthFar = main.depthFar;
		state.projectionMatrix.set(this.projection);
		state.viewRotationMatrix.set(this.viewRotation);
		state.entityRenderState = main.entityRenderState;
		return frustum;
	}

	private void shadeFog(FogData main, float distance) {
		float span = Mth.clamp(distance * FOG_SPAN_FRACTION, MIN_FOG_SPAN, MAX_FOG_SPAN);
		FogData fog = this.fogData;
		fog.color.set(main.color);
		fog.environmentalStart = main.environmentalStart;
		fog.environmentalEnd = main.environmentalEnd;
		fog.renderDistanceStart = Math.min(main.renderDistanceStart, distance - span);
		fog.renderDistanceEnd = Math.min(main.renderDistanceEnd, distance);
		fog.skyEnd = Math.min(main.skyEnd, distance);
		fog.cloudEnd = main.cloudEnd;
	}

	private void extract(Minecraft minecraft, LevelRenderState mainLevel, Frustum frustum) {
		LevelRenderState state = this.levelState;
		state.gameTime = mainLevel.gameTime;
		state.worldPartialTicks = mainLevel.worldPartialTicks;
		state.entityRenderStates.clear();
		state.blockEntityRenderStates.clear();
		LevelExtractorInvoker extractor = (LevelExtractorInvoker) minecraft.levelExtractor;
		extractor.rikka$extractVisibleEntities(this.camera, frustum, minecraft.getDeltaTracker(), state);
		extractor.rikka$extractBlockDestroyAnimation(this.camera, state);
		mainLevel.particlesRenderState.reset();
		minecraft.particleEngine.extract(state.particlesRenderState, new Frustum(frustum).offset(PARTICLE_FRUSTUM_OFFSET), this.camera, state.worldPartialTicks);
		this.sections.clear();
		this.nearbySections.clear();
		minecraft.levelRenderer.sectionOcclusionGraph().addSectionsInFrustum(frustum, this.sections, this.nearbySections);
		if (!this.sections.equals(this.builtSections)) {
			this.builtSections.clear();
			this.builtSections.addAll(this.sections);
			this.sectionsVersion++;
		}
	}

	private TerrainView terrain(TerrainRenderer renderer) {
		TerrainView view = this.terrain;
		if (view == null || this.terrainRenderer != renderer) {
			closeTerrain();
			view = renderer.createView();
			this.terrain = view;
			this.terrainRenderer = renderer;
		}
		return view;
	}

	private void closeTerrain() {
		if (this.terrain != null) {
			this.terrain.close();
			this.terrain = null;
			this.terrainRenderer = null;
		}
	}

	private void drawLevel(Minecraft minecraft, Frame frame, LevelRenderState mainLevel, OptionsRenderState options, GpuBufferSlice fogBuffer) {
		GameRenderer gameRenderer = minecraft.gameRenderer;
		TextureTarget target = frame.target();
		CommandEncoder encoder = RenderSystem.getDevice().createCommandEncoder();
		Vector4f fogColor = this.fogData.color;
		encoder.clearColorAndDepthTextures(
			target.getColorTexture(),
			new Vector4f(fogColor.x(), fogColor.y(), fogColor.z(), 1.0F),
			target.getDepthTexture(),
			0.0
		);
		if (frame.main().skyAllowed() && skyVisible(frame.main().camera(), mainLevel)) {
			sky(target).render(fogBuffer, mainLevel.skyRenderState);
		}

		LevelRenderer levelRenderer = minecraft.levelRenderer;
		RearviewLevel level = (RearviewLevel) levelRenderer;
		TerrainRenderer terrainRenderer = TerrainRenderer.get();
		TerrainView terrain = this.terrain(terrainRenderer);
		terrainRenderer.prepare(terrain, levelRenderer, this.sections, this.sectionsVersion, this.projection, this.viewRotation, this.cameraState.pos, this.fogData);
		level.rikka$overrideSections(this.sections);
		try {
			((LevelExtractorInvoker) minecraft.levelExtractor).rikka$extractVisibleBlockEntities(this.camera, mainLevel.worldPartialTicks, this.levelState);
		} finally {
			level.rikka$overrideSections(null);
		}
		level.rikka$submitFeatures(this.levelState, this.submits);
		this.levelState.entityRenderStates.clear();
		this.levelState.blockEntityRenderStates.clear();
		for (SubmitNodeCollection collection : this.submits.getSubmitsPerOrder().values()) {
			collection.outline.clear();
		}

		try (FeatureRenderDispatcher.PreparedFrame features = gameRenderer.featureRenderDispatcher().prepareFrame(this.submits)) {
			RenderSystem.resizeAllAutoStorageIndexBuffers();
			RenderSystem.setShaderFog(fogBuffer);
			gameRenderer.lighting().setupFor(Lighting.Entry.LEVEL);
			GpuTextureView atlas = minecraft.getAtlasManager().getAtlasOrThrow(AtlasIds.BLOCKS).getTextureView();
			boolean wireframe = mainLevel.renderWireframeTerrain;
			try (RenderPass pass = encoder.createRenderPass(
				() -> LABEL,
				target.getColorTextureView(),
				Optional.empty(),
				target.getDepthTextureView(),
				OptionalDouble.empty()
			)) {
				RenderSystem.bindDefaultUniforms(pass);
				terrain.draw(ChunkSectionLayerGroup.OPAQUE, pass, frame.terrainSampler(), atlas, wireframe);
				features.executeSolid(pass);
				features.executeTranslucent(pass);
				terrain.draw(ChunkSectionLayerGroup.TRANSLUCENT, pass, frame.terrainSampler(), atlas, wireframe);
				features.executeTranslucentAfterTerrain(pass);
				drawSurroundings(minecraft.levelRenderer, pass, frame.main().camera(), mainLevel, options);
			}
			if (features.hasAnySeeThrough()) {
				RenderPassDescriptor seeThrough = RenderPassDescriptor.builder(() -> SEE_THROUGH_LABEL)
					.withColorAttachment(target.getColorTextureView())
					.build();
				try (RenderPass pass = encoder.createRenderPass(seeThrough)) {
					RenderSystem.bindDefaultUniforms(pass);
					features.executeSeeThrough(pass);
				}
			}
		}
	}

	private void drawSurroundings(LevelRenderer levelRenderer, RenderPass pass, CameraRenderState main, LevelRenderState mainLevel, OptionsRenderState options) {
		Vec3 position = this.cameraState.pos;
		Matrix4fStack modelView = RenderSystem.getModelViewStack();
		modelView.pushMatrix();
		try {
			modelView.translate((float) (main.pos.x - position.x), (float) (main.pos.y - position.y), (float) (main.pos.z - position.z));
			if (options.cloudStatus != CloudStatus.OFF && ARGB.alpha(mainLevel.cloudColor) > 0) {
				levelRenderer.cloudRenderer().render(options.cloudStatus, pass);
			}
			levelRenderer.weatherEffectRenderer().render(mainLevel.weatherRenderState, pass);
		} finally {
			modelView.popMatrix();
		}
		levelRenderer.worldBorderRenderer()
			.render(mainLevel.worldBorderRenderState, pass, position, options.renderDistance * SectionPos.SECTION_SIZE);
	}

	private static boolean skyVisible(CameraRenderState camera, LevelRenderState level) {
		FogType fogType = camera.fogType;
		return fogType != FogType.POWDER_SNOW
			&& fogType != FogType.LAVA
			&& !camera.entityRenderState.doesMobEffectBlockSky
			&& level.skyRenderState.skybox != DimensionType.Skybox.NONE;
	}

	private SkyRenderer sky(TextureTarget target) {
		SkyRenderer sky = this.sky;
		if (sky == null) {
			Minecraft minecraft = Minecraft.getInstance();
			sky = new SkyRenderer(minecraft.getTextureManager(), minecraft.getAtlasManager(), target);
			this.sky = sky;
		}
		return sky;
	}

	private FogRenderer fog() {
		FogRenderer fog = this.fog;
		if (fog == null) {
			fog = new FogRenderer();
			this.fog = fog;
		}
		return fog;
	}

	private GlobalSettingsUniform globals() {
		GlobalSettingsUniform globals = this.globals;
		if (globals == null) {
			globals = new GlobalSettingsUniform();
			this.globals = globals;
		}
		return globals;
	}

	private ProjectionMatrixBuffer projectionBuffer() {
		ProjectionMatrixBuffer buffer = this.projectionBuffer;
		if (buffer == null) {
			buffer = new ProjectionMatrixBuffer(LABEL);
			this.projectionBuffer = buffer;
		}
		return buffer;
	}

	private void closeSky() {
		if (this.sky != null) {
			this.sky.close();
			this.sky = null;
		}
	}

	record Request(int width, int height, int fov, int distance, boolean followPitch, int frameRate) {
	}

	record MainView(CameraRenderState camera, boolean skyAllowed) {
	}

	private record Frame(Request request, MainView main, TextureTarget target, Entity entity, GpuSampler terrainSampler) {
	}
}
