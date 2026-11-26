package sh.rikka.client.mixin.simulation;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import java.util.Queue;
import net.minecraft.network.PacketProcessor;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.simulation.packet.ChunkPacketPacer;
import sh.rikka.client.simulation.packet.PacedPacketProcessor;

@Mixin(PacketProcessor.class)
abstract class PacketProcessorMixin implements PacedPacketProcessor {
	@Unique
	private @Nullable ChunkPacketPacer rikka$pacer;

	@Shadow
	public abstract void processQueuedPackets();

	@Override
	public void rikka$processPaced(ChunkPacketPacer pacer) {
		pacer.beginFrame();
		this.rikka$pacer = pacer;
		try {
			this.processQueuedPackets();
		} finally {
			this.rikka$pacer = null;
		}
	}

	@WrapOperation(method = "processQueuedPackets", at = @At(value = "INVOKE", target = "Ljava/util/Queue;isEmpty()Z"))
	private boolean rikka$holdChunkPackets(Queue<?> queue, Operation<Boolean> original) {
		if (original.call(queue)) {
			return true;
		}
		ChunkPacketPacer pacer = this.rikka$pacer;
		return pacer != null && pacer.holds(((QueuedPacketAccessor) queue.element()).rikka$packet());
	}
}
