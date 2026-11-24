package sh.rikka.client.mixin;

import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.client.renderer.state.level.LevelRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

@Mixin(LevelExtractor.class)
public interface LevelExtractorInvoker {
	@Invoker("extractVisibleEntities")
	void rikka$extractVisibleEntities(Camera camera, Frustum frustum, DeltaTracker deltaTracker, LevelRenderState output);

	@Invoker("extractVisibleBlockEntities")
	void rikka$extractVisibleBlockEntities(Camera camera, float partialTick, LevelRenderState output);

	@Invoker("extractBlockDestroyAnimation")
	void rikka$extractBlockDestroyAnimation(Camera camera, LevelRenderState output);
}
