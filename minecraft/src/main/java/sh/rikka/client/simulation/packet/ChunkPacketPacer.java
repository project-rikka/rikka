package sh.rikka.client.simulation.packet;

import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.game.ClientboundLevelChunkWithLightPacket;

public final class ChunkPacketPacer {
	private static final long MINIMUM_SLICE_NANOS = 250_000L;
	private static final int FRAME_SHARE = 2;
	private static final int PERIOD_GROWTH_SHARE = 4;

	private long frameStarted = System.nanoTime();
	private long period = MINIMUM_SLICE_NANOS * FRAME_SHARE;
	private long sliceEnd;

	public void beginFrame() {
		long now = System.nanoTime();
		this.period = Math.min(now - this.frameStarted, this.period + this.period / PERIOD_GROWTH_SHARE);
		this.frameStarted = now;
		this.sliceEnd = now + Math.max(MINIMUM_SLICE_NANOS, this.period / FRAME_SHARE);
	}

	public boolean holds(Packet<?> next) {
		return next instanceof ClientboundLevelChunkWithLightPacket && System.nanoTime() >= this.sliceEnd;
	}
}
