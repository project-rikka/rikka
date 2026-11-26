package sh.rikka.client.mixin.culling;

import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

@Mixin(EntityRenderer.class)
public interface EntityRendererInvoker {
	@Invoker("shouldShowName")
	boolean rikka$shouldShowName(Entity entity, double distanceToCameraSq);
}
