package sh.rikka.client.events;

import net.minecraft.network.protocol.Packet;
import sh.rikka.client.event.CancellableEvent;

public final class PacketEvent {
	private PacketEvent() {
	}

	public static final class Send extends CancellableEvent {
		private final Packet<?> packet;

		Send(Packet<?> packet) {
			this.packet = packet;
		}

		public Packet<?> packet() {
			return this.packet;
		}
	}

	public static final class Receive extends CancellableEvent {
		private final Packet<?> packet;

		Receive(Packet<?> packet) {
			this.packet = packet;
		}

		public Packet<?> packet() {
			return this.packet;
		}
	}
}
