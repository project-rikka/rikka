package sh.rikka.client.mixin.culling;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.LevelChunkSection;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.render.culling.OcclusionCulling;

@Mixin(LevelChunk.class)
abstract class LevelChunkMixin {
	@Shadow
	@Final
	private Level level;

	@WrapOperation(
		method = "setBlockState(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;I)Lnet/minecraft/world/level/block/state/BlockState;",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/world/level/chunk/LevelChunkSection;setBlockState(IIILnet/minecraft/world/level/block/state/BlockState;)Lnet/minecraft/world/level/block/state/BlockState;"
		)
	)
	private BlockState rikka$occupancyChanged(
		LevelChunkSection section,
		int x,
		int y,
		int z,
		BlockState state,
		Operation<BlockState> original,
		@Local(argsOnly = true) BlockPos pos
	) {
		BlockState previous = original.call(section, x, y, z, state);
		if (this.level instanceof ClientLevel client) {
			OcclusionCulling.blockChanged(client, pos, previous, state);
		}
		return previous;
	}
}
