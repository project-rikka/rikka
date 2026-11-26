package sh.rikka.client.mixin.culling;

import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.multiplayer.ClientChunkCache;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.world.level.chunk.LevelChunk;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.render.culling.OcclusionCulling;

@Mixin(ClientChunkCache.class)
abstract class ClientChunkCacheMixin {
	@Shadow
	@Final
	private ClientLevel level;

	@ModifyExpressionValue(
		method = "updateViewRadius",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/client/multiplayer/ClientChunkCache$Storage;inRange(II)Z")
	)
	private boolean rikka$occupancyKept(boolean kept, @Local(name = "chunk") LevelChunk chunk) {
		if (!kept) {
			OcclusionCulling.chunkUnloaded(this.level, chunk.getPos());
		}
		return kept;
	}
}
