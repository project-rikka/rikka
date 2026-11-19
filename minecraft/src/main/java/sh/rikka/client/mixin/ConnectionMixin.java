package sh.rikka.client.mixin;

import io.netty.channel.ChannelFutureListener;
import net.minecraft.network.Connection;
import net.minecraft.network.PacketListener;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.PacketFlow;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;

@Mixin(Connection.class)
abstract class ConnectionMixin {
	@Shadow
	public abstract PacketFlow getReceiving();

	@Inject(
		method = "send(Lnet/minecraft/network/protocol/Packet;Lio/netty/channel/ChannelFutureListener;Z)V",
		at = @At("HEAD"),
		cancellable = true
	)
	private void rikka$send(Packet<?> packet, @Nullable ChannelFutureListener listener, boolean flush, CallbackInfo callback) {
		if (ClientHooks.packetSend(packet, getReceiving())) {
			callback.cancel();
		}
	}

	@Inject(method = "genericsFtw", at = @At("HEAD"), cancellable = true)
	private static void rikka$receive(Packet<?> packet, PacketListener listener, CallbackInfo callback) {
		if (ClientHooks.packetReceive(packet, listener)) {
			callback.cancel();
		}
	}
}
