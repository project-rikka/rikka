package sh.rikka.client.mixin.terrain;

import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.core.BlockPos;
import net.minecraft.core.SectionPos;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.boost.BoostFeature;
import sh.rikka.client.render.terrain.LeafCulling;

@Mixin(LevelExtractor.class)
abstract class LevelExtractorLeafMixin {
	@Shadow
	private void setSectionDirty(int sectionX, int sectionY, int sectionZ, boolean playerChanged) {
	}

	@Inject(method = "setBlockDirty(Lnet/minecraft/core/BlockPos;Z)V", at = @At("TAIL"))
	private void rikka$dirtyLeafReach(BlockPos pos, boolean playerChanged, CallbackInfo callback) {
		if (BoostFeature.LEAF_CULLING.isActive()) {
			rikka$dirtyReach(pos.getX(), pos.getY(), pos.getZ(), pos.getX(), pos.getY(), pos.getZ(), playerChanged);
		}
	}

	@Inject(method = "setBlocksDirty", at = @At("TAIL"))
	private void rikka$dirtyLeafRangeReach(int x0, int y0, int z0, int x1, int y1, int z1, CallbackInfo callback) {
		if (BoostFeature.LEAF_CULLING.isActive()) {
			rikka$dirtyReach(x0, y0, z0, x1, y1, z1, false);
		}
	}

	@Unique
	private void rikka$dirtyReach(int x0, int y0, int z0, int x1, int y1, int z1, boolean playerChanged) {
		int reach = LeafCulling.REACH;
		for (int z = SectionPos.blockToSectionCoord(z0 - reach); z <= SectionPos.blockToSectionCoord(z1 + reach); z++) {
			for (int x = SectionPos.blockToSectionCoord(x0 - reach); x <= SectionPos.blockToSectionCoord(x1 + reach); x++) {
				for (int y = SectionPos.blockToSectionCoord(y0 - reach); y <= SectionPos.blockToSectionCoord(y1 + reach); y++) {
					setSectionDirty(x, y, z, playerChanged);
				}
			}
		}
	}
}
