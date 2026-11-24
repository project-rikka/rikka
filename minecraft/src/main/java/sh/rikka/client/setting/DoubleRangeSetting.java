package sh.rikka.client.setting;

import com.mojang.serialization.Codec;

public final class DoubleRangeSetting extends Setting<DoubleRange, DoubleRangeSetting> {
	private final DoubleRange defaultValue;
	private final double min;
	private final double max;
	private double step;
	private volatile DoubleRange value;

	DoubleRangeSetting(String id, String translationKey, DoubleRange defaultValue, double min, double max) {
		super(id, translationKey);
		Numeric.requireBounds(min, max);
		if (defaultValue.low() < min || defaultValue.high() > max) {
			throw new IllegalArgumentException("default " + defaultValue + " of " + id + " is outside " + min + ".." + max);
		}
		this.defaultValue = defaultValue;
		this.min = min;
		this.max = max;
		this.value = defaultValue;
	}

	public DoubleRangeSetting step(double step) {
		Numeric.requireStep(step, this.min, this.max);
		if (!snap(this.defaultValue, step).equals(this.defaultValue)) {
			throw new IllegalArgumentException("default " + this.defaultValue + " of " + id() + " is not on step " + step);
		}
		this.step = step;
		return this;
	}

	public DoubleRange get() {
		return this.value;
	}

	public double low() {
		return this.value.low();
	}

	public double high() {
		return this.value.high();
	}

	public void set(DoubleRange value) {
		DoubleRange snapped = snap(value, this.step);
		if (this.value.equals(snapped)) {
			return;
		}
		this.value = snapped;
		changed(snapped);
	}

	public void set(double low, double high) {
		set(new DoubleRange(Math.min(low, high), Math.max(low, high)));
	}

	private DoubleRange snap(DoubleRange range, double step) {
		return new DoubleRange(Numeric.snap(range.low(), this.min, this.max, step), Numeric.snap(range.high(), this.min, this.max, step));
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
	public DoubleRange value() {
		return this.value;
	}

	@Override
	public DoubleRange defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(DoubleRange value) {
		set(value);
	}

	@Override
	public Codec<DoubleRange> codec() {
		return DoubleRange.CODEC;
	}
}
