package sh.rikka.client.mixin.simulation;

import net.minecraft.network.protocol.Packet;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(targets = "net.minecraft.network.PacketProcessor$ListenerAndPacket")
public interface QueuedPacketAccessor {
	@Accessor("packet")
	Packet<?> rikka$packet();
}
