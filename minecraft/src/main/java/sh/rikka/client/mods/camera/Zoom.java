package sh.rikka.client.mods.camera;

import net.minecraft.util.Util;

final class Zoom {
	private static final double NANOS_PER_SECOND = 1.0e9;
	private static final double MAX_STEP_SECONDS = 0.1;
	private static final double SETTLED_DISTANCE = 1.0e-4;
	private static final double SETTLED_VELOCITY = 1.0e-3;

	private double exponent;
	private double velocity;
	private double target;
	private double rate;
	private boolean scaleSensitivity;
	private long lastNanos = Util.getNanos();

	double target() {
		return this.target;
	}

	void aim(double target, double rate, boolean scaleSensitivity) {
		this.target = target;
		this.rate = rate;
		this.scaleSensitivity = scaleSensitivity;
		if (rate <= 0.0) {
			this.exponent = target;
			this.velocity = 0.0;
		}
	}

	double sensitivity() {
		return this.scaleSensitivity ? Math.exp(-this.exponent) : 1.0;
	}

	float fov(float vanilla) {
		advance();
		if (this.exponent == 0.0) {
			return vanilla;
		}
		double halfAngle = Math.toRadians(vanilla * 0.5);
		return (float) Math.toDegrees(2.0 * Math.atan(Math.tan(halfAngle) * Math.exp(-this.exponent)));
	}

	private void advance() {
		long now = Util.getNanos();
		double seconds = Math.min((now - this.lastNanos) / NANOS_PER_SECOND, MAX_STEP_SECONDS);
		this.lastNanos = now;
		double offset = this.exponent - this.target;
		if (offset == 0.0 && this.velocity == 0.0) {
			return;
		}
		double decay = Math.exp(-this.rate * seconds);
		double impulse = (this.velocity + this.rate * offset) * seconds;
		this.exponent = this.target + (offset + impulse) * decay;
		this.velocity = (this.velocity - this.rate * impulse) * decay;
		if (Math.abs(this.exponent - this.target) < SETTLED_DISTANCE && Math.abs(this.velocity) < SETTLED_VELOCITY) {
			this.exponent = this.target;
			this.velocity = 0.0;
		}
	}
}
