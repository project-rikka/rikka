package sh.rikka.client.mixin.entity;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.render.entity.DistantEntities;

@Mixin(LevelExtractor.class)
abstract class EntityExtractionMixin {
	@WrapOperation(
		method = "extractVisibleEntities",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/extract/LevelExtractor;extractEntity(Lnet/minecraft/world/entity/Entity;F)Lnet/minecraft/client/renderer/entity/state/EntityRenderState;"
		)
	)
	private EntityRenderState rikka$retainDistantState(
		LevelExtractor extractor,
		Entity entity,
		float partialTick,
		Operation<EntityRenderState> original,
		@Local(ordinal = 0) double cameraX,
		@Local(ordinal = 1) double cameraY,
		@Local(ordinal = 2) double cameraZ
	) {
		EntityRenderState retained = DistantEntities.retained(entity, cameraX, cameraY, cameraZ);
		if (retained != null) {
			return retained;
		}
		EntityRenderState state = original.call(extractor, entity, partialTick);
		DistantEntities.retain(entity, state);
		return state;
	}
}
