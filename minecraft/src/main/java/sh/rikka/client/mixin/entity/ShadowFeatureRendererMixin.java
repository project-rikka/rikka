package sh.rikka.client.mixin.entity;

import net.minecraft.client.renderer.feature.ShadowFeatureRenderer;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.shapes.VoxelShape;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

@Mixin(ShadowFeatureRenderer.class)
abstract class ShadowFeatureRendererMixin {
	@Unique
	private @Nullable VoxelShape rikka$boundedShape;

	@Unique
	private @Nullable AABB rikka$bounds;

	@Redirect(method = "prepare", at = @At(value = "INVOKE", target = "Lnet/minecraft/world/phys/shapes/VoxelShape;bounds()Lnet/minecraft/world/phys/AABB;"))
	private AABB rikka$shapeBounds(VoxelShape shape) {
		AABB bounds = this.rikka$bounds;
		if (shape != this.rikka$boundedShape || bounds == null) {
			bounds = shape.bounds();
			this.rikka$boundedShape = shape;
			this.rikka$bounds = bounds;
		}
		return bounds;
	}
}
