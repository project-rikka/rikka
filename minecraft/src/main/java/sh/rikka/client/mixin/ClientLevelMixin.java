package sh.rikka.client.mixin;

import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;

@Mixin(ClientLevel.class)
abstract class ClientLevelMixin {
	@Inject(method = "onChunkLoaded", at = @At("TAIL"))
	private void rikka$chunkLoaded(ChunkPos pos, CallbackInfo callback) {
		ClientHooks.chunkLoaded(pos.x(), pos.z());
	}

	@Inject(method = "sendBlockUpdated", at = @At("HEAD"))
	private void rikka$blockUpdated(BlockPos pos, BlockState old, BlockState current, int updateFlags, CallbackInfo callback) {
		ClientHooks.blockUpdated(pos);
	}
}
