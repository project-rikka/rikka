package sh.rikka.client.render.terrain;

import net.minecraft.util.Util;

public final class CompileFeed {
	private static final int CEILING_PER_WORKER = 64;

	private final int floor = Util.maxAllowedExecutorThreads();
	private final int ceiling = this.floor * CEILING_PER_WORKER;
	private int window = this.floor;
	private int remaining;
	private boolean exhausted;

	public void beginFrame(int queued) {
		if (queued == 0 && this.exhausted) {
			this.window = Math.min(this.window * 2, this.ceiling);
		} else if (queued > this.window / 2) {
			this.window = Math.max(this.window / 2, this.floor);
		}
		this.remaining = Math.max(0, this.window - queued);
		this.exhausted = false;
	}

	public boolean hasRoom() {
		return this.remaining > 0;
	}

	public void admitted() {
		this.remaining--;
		this.exhausted = this.remaining <= 0;
	}
}
