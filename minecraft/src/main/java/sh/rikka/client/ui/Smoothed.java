package sh.rikka.client.ui;

import net.minecraft.util.Util;

public final class Smoothed {
	private static final float NANOS_PER_MILLI = 1.0E6F;
	private static final float MAX_STEP_MILLIS = 100.0F;

	private final float timeConstantMillis;
	private final float epsilon;
	private float value;
	private float target;
	private long updatedAt;

	public Smoothed(float timeConstantMillis, float epsilon, float initial) {
		if (!(timeConstantMillis > 0.0F)) {
			throw new IllegalArgumentException("time constant must be positive: " + timeConstantMillis);
		}
		this.timeConstantMillis = timeConstantMillis;
		this.epsilon = epsilon;
		this.value = initial;
		this.target = initial;
		this.updatedAt = Util.getNanos();
	}

	public float update(float target) {
		return update(target, this.timeConstantMillis);
	}

	public float update(float target, float timeConstantMillis) {
		long now = Util.getNanos();
		float elapsed = Math.clamp((now - this.updatedAt) / NANOS_PER_MILLI, 0.0F, MAX_STEP_MILLIS);
		this.updatedAt = now;
		this.target = target;
		this.value = target + (this.value - target) * (float) Math.exp(-elapsed / Math.max(timeConstantMillis, 1.0F));
		if (Math.abs(this.value - target) <= this.epsilon) {
			this.value = target;
		}
		return this.value;
	}

	public void snap(float value) {
		this.value = value;
		this.target = value;
		this.updatedAt = Util.getNanos();
	}

	public void setValue(float value) {
		this.value = value;
	}

	public float value() {
		return this.value;
	}

	public boolean isSettled() {
		return this.value == this.target;
	}
}
