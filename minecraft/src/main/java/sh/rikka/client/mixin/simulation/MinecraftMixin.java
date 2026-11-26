package sh.rikka.client.mixin.simulation;

import net.minecraft.client.Minecraft;
import net.minecraft.network.PacketProcessor;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;
import sh.rikka.client.simulation.packet.ChunkPacketPacer;
import sh.rikka.client.simulation.packet.PacedPacketProcessor;

@Mixin(Minecraft.class)
abstract class MinecraftMixin {
	@Unique
	private final ChunkPacketPacer rikka$chunkPacketPacer = new ChunkPacketPacer();

	@Redirect(method = "runTick", at = @At(value = "INVOKE", target = "Lnet/minecraft/network/PacketProcessor;processQueuedPackets()V"))
	private void rikka$paceChunkPackets(PacketProcessor processor) {
		((PacedPacketProcessor) processor).rikka$processPaced(this.rikka$chunkPacketPacer);
	}
}
