package sh.rikka.client.mods.rearview;

import net.minecraft.util.Util;

final class FramePacer {
	private static final long NANOS_PER_SECOND = 1_000_000_000L;

	private boolean invalid = true;
	private long nextFrameNanos;

	void invalidate() {
		this.invalid = true;
	}

	boolean due(int framesPerSecond) {
		long now = Util.getNanos();
		if (!this.invalid && now - this.nextFrameNanos < 0) {
			return false;
		}
		long interval = NANOS_PER_SECOND / framesPerSecond;
		boolean behind = now - this.nextFrameNanos >= interval;
		this.nextFrameNanos = this.invalid || behind ? now + interval : this.nextFrameNanos + interval;
		this.invalid = false;
		return true;
	}
}
