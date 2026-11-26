package sh.rikka.client.mixin.culling;

import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.chunk.LevelChunk;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.render.culling.OcclusionCulling;

@Mixin(ClientLevel.class)
abstract class ClientLevelMixin {
	@Inject(method = "onChunkLoaded", at = @At("HEAD"))
	private void rikka$occupancyLoaded(ChunkPos pos, CallbackInfo callback) {
		OcclusionCulling.chunkLoaded((ClientLevel) (Object) this, pos);
	}

	@Inject(method = "unload", at = @At("HEAD"))
	private void rikka$occupancyUnloaded(LevelChunk chunk, CallbackInfo callback) {
		OcclusionCulling.chunkUnloaded((ClientLevel) (Object) this, chunk.getPos());
	}
}
