package sh.rikka.client.mixin.culling;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.world.phys.AABB;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.render.culling.OcclusionCulling;

@Mixin(EntityRenderer.class)
abstract class EntityRendererMixin {
	@WrapOperation(
		method = "shouldRender",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/culling/Frustum;isVisible(Lnet/minecraft/world/phys/AABB;)Z", ordinal = 0)
	)
	private boolean rikka$cullingBox(Frustum frustum, AABB box, Operation<Boolean> original) {
		OcclusionCulling.cullingBox(box);
		return original.call(frustum, box);
	}
}
