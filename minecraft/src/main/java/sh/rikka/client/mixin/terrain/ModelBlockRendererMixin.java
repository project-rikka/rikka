package sh.rikka.client.mixin.terrain;

import com.llamalad7.mixinextras.injector.ModifyReturnValue;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.renderer.block.BlockAndTintGetter;
import net.minecraft.client.renderer.block.ModelBlockRenderer;
import net.minecraft.client.renderer.chunk.RenderSectionRegion;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.boost.BoostFeature;
import sh.rikka.client.render.terrain.LeafCulling;

@Mixin(ModelBlockRenderer.class)
abstract class ModelBlockRendererMixin {
	@ModifyReturnValue(method = "shouldRenderFace", at = @At("RETURN"))
	private boolean rikka$cullLeaves(
		boolean rendered,
		@Local(argsOnly = true) BlockAndTintGetter level,
		@Local(argsOnly = true) BlockState state,
		@Local(argsOnly = true) Direction direction,
		@Local(argsOnly = true) BlockPos neighborPos
	) {
		return rendered && !(BoostFeature.LEAF_CULLING.isActive() && level instanceof RenderSectionRegion && LeafCulling.hidden(level, state, direction, neighborPos));
	}
}
