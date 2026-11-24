package sh.rikka.client.setting;

import com.mojang.serialization.Codec;

public final class DoubleSetting extends Setting<Double, DoubleSetting> {
	private final double defaultValue;
	private final double min;
	private final double max;
	private double step;
	private volatile double value;

	DoubleSetting(String id, String translationKey, double defaultValue, double min, double max) {
		super(id, translationKey);
		Numeric.requireBounds(min, max);
		Numeric.requireFinite(defaultValue);
		if (defaultValue < min || defaultValue > max) {
			throw new IllegalArgumentException("default " + defaultValue + " of " + id + " is outside " + min + ".." + max);
		}
		this.defaultValue = defaultValue;
		this.min = min;
		this.max = max;
		this.value = defaultValue;
	}

	public DoubleSetting step(double step) {
		Numeric.requireStep(step, this.min, this.max);
		if (Numeric.snap(this.defaultValue, this.min, this.max, step) != this.defaultValue) {
			throw new IllegalArgumentException("default " + this.defaultValue + " of " + id() + " is not on step " + step);
		}
		this.step = step;
		return this;
	}

	public double get() {
		return this.value;
	}

	public float getFloat() {
		return (float) this.value;
	}

	public void set(double value) {
		double snapped = Numeric.snap(value, this.min, this.max, this.step);
		if (this.value == snapped) {
			return;
		}
		this.value = snapped;
		changed(snapped);
	}

	public double min() {
		return this.min;
	}

	public double max() {
		return this.max;
	}

	public double step() {
		return this.step;
	}

	@Override
	public Double value() {
		return this.value;
	}

	@Override
	public Double defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(Double value) {
		set(value);
	}

	@Override
	public Codec<Double> codec() {
		return Numeric.FINITE_DOUBLE;
	}
}
