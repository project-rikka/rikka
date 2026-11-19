package sh.rikka.client.mods.hud;

import java.util.OptionalLong;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.network.protocol.ping.ClientboundPongResponsePacket;
import net.minecraft.network.protocol.ping.ServerboundPingRequestPacket;
import net.minecraft.util.Util;
import org.jspecify.annotations.Nullable;

final class LatencyProbe {
	private static final long INTERVAL_MILLIS = 1000;
	private static final long STALE_MILLIS = 5000;

	private final Set<Long> outstanding = ConcurrentHashMap.newKeySet();
	private volatile @Nullable Sample sample;
	private @Nullable ClientPacketListener connection;
	private long nextRequestAt = Long.MIN_VALUE;

	void tick(@Nullable ClientPacketListener current) {
		if (current != this.connection) {
			reset();
			this.connection = current;
		}
		long now = Util.getMillis();
		if (current == null || now < this.nextRequestAt) {
			return;
		}
		this.nextRequestAt = now + INTERVAL_MILLIS;
		this.outstanding.removeIf(sentAt -> now - sentAt > STALE_MILLIS);
		this.outstanding.add(now);
		current.send(new ServerboundPingRequestPacket(now));
	}

	void receive(ClientboundPongResponsePacket pong) {
		if (this.outstanding.remove(pong.time())) {
			long now = Util.getMillis();
			this.sample = new Sample(now - pong.time(), now);
		}
	}

	OptionalLong roundTripMillis() {
		Sample current = this.sample;
		if (current == null || Util.getMillis() - current.receivedAt() > STALE_MILLIS) {
			return OptionalLong.empty();
		}
		return OptionalLong.of(current.roundTripMillis());
	}

	void reset() {
		this.outstanding.clear();
		this.sample = null;
		this.connection = null;
		this.nextRequestAt = Long.MIN_VALUE;
	}

	private record Sample(long roundTripMillis, long receivedAt) {
	}
}
