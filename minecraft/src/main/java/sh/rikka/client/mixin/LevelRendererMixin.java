package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.renderpearl.api.commands.RenderPass;
import com.mojang.renderpearl.api.textures.GpuSampler;
import it.unimi.dsi.fastutil.objects.ObjectArrayList;
import java.util.Optional;
import java.util.OptionalDouble;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.LevelTargetBundle;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.chunk.ChunkSectionsToRender;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.client.renderer.feature.FeatureRenderDispatcher;
import net.minecraft.client.renderer.state.level.LevelRenderState;
import org.jspecify.annotations.Nullable;
import org.objectweb.asm.Opcodes;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.motionblur.MotionBlurHooks;
import sh.rikka.client.mods.rearview.RearviewLevel;

@Mixin(LevelRenderer.class)
abstract class LevelRendererMixin implements RearviewLevel {
	private static final String SOLID_FEATURES =
		"Lnet/minecraft/client/renderer/feature/FeatureRenderDispatcher$PreparedFrame;executeSolid(Lcom/mojang/renderpearl/api/commands/RenderPass;)V";
	private static final String VISIBLE_SECTIONS =
		"Lnet/minecraft/client/renderer/LevelRenderer;visibleSections:Lit/unimi/dsi/fastutil/objects/ObjectArrayList;";
	private static final String NEARBY_VISIBLE_SECTIONS =
		"Lnet/minecraft/client/renderer/LevelRenderer;nearbyVisibleSections:Lit/unimi/dsi/fastutil/objects/ObjectArrayList;";

	@Shadow
	@Final
	private LevelTargetBundle targets;

	@Shadow
	@Final
	private GameRenderer gameRenderer;

	@Shadow
	private @Nullable GpuSampler chunkLayerSampler;

	@Unique
	private @Nullable RenderPass rikka$featurePass;

	@Unique
	private @Nullable ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$sectionOverride;

	@Unique
	private @Nullable ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$followedSections;

	@Unique
	private @Nullable ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$followedNearbySections;

	@Unique
	private final ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$sectionsToUpdate = new ObjectArrayList<>();

	@Unique
	private final ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$sectionsToResort = new ObjectArrayList<>();

	@Shadow
	private void submitEntities(PoseStack poseStack, LevelRenderState levelRenderState, SubmitNodeCollector output) {
	}

	@Shadow
	private void submitBlockEntities(PoseStack poseStack, LevelRenderState levelRenderState, SubmitNodeCollector submitNodeCollector) {
	}

	@Shadow
	private void submitBlockDestroyAnimation(PoseStack poseStack, SubmitNodeCollector submitNodeCollector, LevelRenderState levelRenderState) {
	}

	@Shadow
	private void submitTransientBlocks(PoseStack poseStack, SubmitNodeCollector submitNodeCollector, LevelRenderState levelRenderState) {
	}

	@Override
	public void rikka$overrideSections(@Nullable ObjectArrayList<SectionRenderDispatcher.RenderSection> sections) {
		this.rikka$sectionOverride = sections;
	}

	@Override
	public void rikka$followSections(
		@Nullable ObjectArrayList<SectionRenderDispatcher.RenderSection> visible,
		@Nullable ObjectArrayList<SectionRenderDispatcher.RenderSection> nearby
	) {
		this.rikka$followedSections = visible;
		this.rikka$followedNearbySections = nearby;
	}

	@Override
	public ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$sectionsToUpdate(ObjectArrayList<SectionRenderDispatcher.RenderSection> visible) {
		return rikka$join(visible, this.rikka$followedSections, this.rikka$sectionsToUpdate);
	}

	@Override
	public @Nullable GpuSampler rikka$chunkLayerSampler() {
		return this.chunkLayerSampler;
	}

	@Override
	public void rikka$submitFeatures(LevelRenderState state, SubmitNodeCollector collector) {
		PoseStack poseStack = new PoseStack();
		submitEntities(poseStack, state, collector);
		submitBlockEntities(poseStack, state, collector);
		submitBlockDestroyAnimation(poseStack, collector, state);
		submitTransientBlocks(poseStack, collector, state);
		state.particlesRenderState.submit(collector, state.cameraRenderState);
	}

	@ModifyExpressionValue(method = "extractSectionDrawGroups", at = @At(value = "FIELD", target = VISIBLE_SECTIONS, opcode = Opcodes.GETFIELD))
	private ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$drawnSections(ObjectArrayList<SectionRenderDispatcher.RenderSection> sections) {
		return this.rikka$sectionOverride != null ? this.rikka$sectionOverride : sections;
	}

	@ModifyReturnValue(method = "visibleSections", at = @At("RETURN"))
	private ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$visibleSections(ObjectArrayList<SectionRenderDispatcher.RenderSection> sections) {
		return this.rikka$sectionOverride != null ? this.rikka$sectionOverride : sections;
	}

	@ModifyExpressionValue(
		method = "scheduleTranslucentSectionResort",
		at = @At(value = "FIELD", target = NEARBY_VISIBLE_SECTIONS, opcode = Opcodes.GETFIELD)
	)
	private ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$resortedSections(ObjectArrayList<SectionRenderDispatcher.RenderSection> nearby) {
		return rikka$join(nearby, this.rikka$followedNearbySections, this.rikka$sectionsToResort);
	}

	@Inject(method = {"invalidateCompiledGeometry", "resetLevelRenderData"}, at = @At("TAIL"))
	private void rikka$forgetFollowedSections(CallbackInfo callback) {
		if (this.rikka$followedSections != null) {
			this.rikka$followedSections.clear();
		}
		if (this.rikka$followedNearbySections != null) {
			this.rikka$followedNearbySections.clear();
		}
	}

	@Unique
	private static ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$join(
		ObjectArrayList<SectionRenderDispatcher.RenderSection> own,
		@Nullable ObjectArrayList<SectionRenderDispatcher.RenderSection> followed,
		ObjectArrayList<SectionRenderDispatcher.RenderSection> joined
	) {
		joined.clear();
		if (followed == null || followed.isEmpty()) {
			return own;
		}
		joined.addAll(own);
		joined.addAll(followed);
		return joined;
	}

	@WrapOperation(method = "executeSolid", at = @At(value = "INVOKE", target = SOLID_FEATURES))
	private void rikka$blurTerrain(FeatureRenderDispatcher.PreparedFrame frame, RenderPass terrainPass, Operation<Void> original) {
		if (!MotionBlurHooks.blursTerrain()) {
			original.call(frame, terrainPass);
			return;
		}
		RenderTarget main = this.targets.main.get();
		terrainPass.close();
		MotionBlurHooks.terrainDrawn(main);
		RenderPass featurePass = RenderSystem.getDevice()
			.createCommandEncoder()
			.createRenderPass(() -> "Solid features", main.getColorTextureView(), Optional.empty(), main.getDepthTextureView(), OptionalDouble.empty());
		RenderSystem.bindDefaultUniforms(featurePass);
		try {
			original.call(frame, featurePass);
		} catch (RuntimeException failure) {
			featurePass.close();
			throw failure;
		}
		if (this.gameRenderer.useImprovedTransparency()) {
			featurePass.close();
		} else {
			this.rikka$featurePass = featurePass;
		}
	}

	@WrapMethod(method = "executeClassicTransparency")
	private void rikka$continueTransparency(
		ChunkSectionsToRender sections,
		FeatureRenderDispatcher.PreparedFrame frame,
		RenderPass pass,
		Operation<Void> original
	) {
		RenderPass featurePass = this.rikka$featurePass;
		if (featurePass == null) {
			original.call(sections, frame, pass);
			return;
		}
		this.rikka$featurePass = null;
		try (featurePass) {
			original.call(sections, frame, featurePass);
		}
	}
}
