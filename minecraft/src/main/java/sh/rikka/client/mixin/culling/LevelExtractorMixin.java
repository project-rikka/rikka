package sh.rikka.client.mixin.culling;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.blockentity.BlockEntityRenderDispatcher;
import net.minecraft.client.renderer.blockentity.state.BlockEntityRenderState;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.block.entity.BlockEntity;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.render.culling.OcclusionCulling;

@Mixin(LevelExtractor.class)
abstract class LevelExtractorMixin {
	@Shadow
	@Final
	private LevelRenderer levelRenderer;

	@Shadow
	private @Nullable ClientLevel level;

	@Inject(
		method = "extract",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/extract/LevelExtractor;extractVisibleEntities(Lnet/minecraft/client/Camera;Lnet/minecraft/client/renderer/culling/Frustum;Lnet/minecraft/client/DeltaTracker;Lnet/minecraft/client/renderer/state/level/LevelRenderState;)V"
		)
	)
	private void rikka$beginOcclusion(DeltaTracker deltaTracker, Camera camera, float worldPartialTicks, CallbackInfo callback) {
		ClientLevel extracted = this.level;
		if (extracted != null) {
			OcclusionCulling.beginExtraction(extracted, camera, this.levelRenderer.entityRenderDispatcher());
		}
	}

	@Inject(
		method = "extract",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/extract/LevelExtractor;extractVisibleBlockEntities(Lnet/minecraft/client/Camera;FLnet/minecraft/client/renderer/state/level/LevelRenderState;)V",
			shift = At.Shift.AFTER
		)
	)
	private void rikka$publishOcclusion(DeltaTracker deltaTracker, Camera camera, float worldPartialTicks, CallbackInfo callback) {
		OcclusionCulling.endExtraction();
	}

	@WrapOperation(
		method = "extractVisibleEntities",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/extract/LevelExtractor;isEntityVisible(Lnet/minecraft/world/entity/Entity;Lnet/minecraft/client/renderer/culling/Frustum;DDDFJ)Z"
		)
	)
	private boolean rikka$occludeEntity(
		LevelExtractor extractor,
		Entity entity,
		Frustum frustum,
		double camX,
		double camY,
		double camZ,
		float partialTicks,
		long chunkFadeDuration,
		Operation<Boolean> original
	) {
		OcclusionCulling.prepareEntity();
		return original.call(extractor, entity, frustum, camX, camY, camZ, partialTicks, chunkFadeDuration) && OcclusionCulling.isEntityVisible(entity);
	}

	@WrapOperation(
		method = "extractVisibleBlockEntities",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/blockentity/BlockEntityRenderDispatcher;tryExtractRenderState(Lnet/minecraft/world/level/block/entity/BlockEntity;FLnet/minecraft/client/renderer/feature/ModelFeatureRenderer$CrumblingOverlay;Z)Lnet/minecraft/client/renderer/blockentity/state/BlockEntityRenderState;"
		)
	)
	private @Nullable BlockEntityRenderState rikka$occludeBlockEntity(
		BlockEntityRenderDispatcher dispatcher,
		BlockEntity blockEntity,
		float partialTicks,
		ModelFeatureRenderer.@Nullable CrumblingOverlay breakProgress,
		boolean isGloballyRendered,
		Operation<BlockEntityRenderState> original
	) {
		if (isGloballyRendered) {
			return original.call(dispatcher, blockEntity, partialTicks, breakProgress, isGloballyRendered);
		}
		if (OcclusionCulling.hidesBlockEntity(dispatcher, blockEntity)) {
			return null;
		}
		BlockEntityRenderState state = original.call(dispatcher, blockEntity, partialTicks, breakProgress, isGloballyRendered);
		if (state != null) {
			OcclusionCulling.blockEntityDrawn(blockEntity);
		}
		return state;
	}

	@Inject(method = "setLevel", at = @At("HEAD"))
	private void rikka$levelChanged(@Nullable ClientLevel newLevel, CallbackInfo callback) {
		OcclusionCulling.levelChanged();
	}
}
