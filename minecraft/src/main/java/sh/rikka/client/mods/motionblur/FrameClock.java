package sh.rikka.client.mods.motionblur;

final class FrameClock {
	private static final double NANOS_PER_SECOND = 1.0e9;
	private static final double MAX_FRAME_SECONDS = 1.0;
	private static final float SMOOTHING = 0.15F;

	private long lastNanos;
	private boolean started;
	private boolean continuous;
	private double seconds;
	private float fps;
	private float smoothedFps;

	void tick(long nanos) {
		double delta = (nanos - this.lastNanos) / NANOS_PER_SECOND;
		this.continuous = this.started && delta > 0.0 && delta < MAX_FRAME_SECONDS;
		this.started = true;
		this.lastNanos = nanos;
		this.seconds = nanos / NANOS_PER_SECOND;
		this.fps = this.continuous ? (float) (1.0 / delta) : 0.0F;
		if (this.fps > 0.0F) {
			this.smoothedFps = this.smoothedFps <= 0.0F ? this.fps : this.smoothedFps + (this.fps - this.smoothedFps) * SMOOTHING;
		}
	}

	void reset() {
		this.started = false;
		this.continuous = false;
		this.fps = 0.0F;
		this.smoothedFps = 0.0F;
	}

	boolean continuous() {
		return this.continuous;
	}

	double seconds() {
		return this.seconds;
	}

	float fps() {
		return this.fps;
	}

	float smoothedFps() {
		return this.smoothedFps;
	}
}
