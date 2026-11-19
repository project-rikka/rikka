package sh.rikka.client.setting;

import com.mojang.serialization.Codec;

public final class IntRangeSetting extends Setting<IntRange, IntRangeSetting> {
	private final IntRange defaultValue;
	private final int min;
	private final int max;
	private int step = 1;
	private volatile IntRange value;

	IntRangeSetting(String id, String translationKey, IntRange defaultValue, int min, int max) {
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

	public IntRangeSetting step(int step) {
		Numeric.requireStep(step, this.min, this.max);
		if (!snap(this.defaultValue, step).equals(this.defaultValue)) {
			throw new IllegalArgumentException("default " + this.defaultValue + " of " + id() + " is not on step " + step);
		}
		this.step = step;
		return this;
	}

	public IntRange get() {
		return this.value;
	}

	public int low() {
		return this.value.low();
	}

	public int high() {
		return this.value.high();
	}

	public void set(IntRange value) {
		IntRange snapped = snap(value, this.step);
		if (this.value.equals(snapped)) {
			return;
		}
		this.value = snapped;
		changed(snapped);
	}

	public void set(int low, int high) {
		set(new IntRange(Math.min(low, high), Math.max(low, high)));
	}

	private IntRange snap(IntRange range, int step) {
		return new IntRange(Numeric.snap(range.low(), this.min, this.max, step), Numeric.snap(range.high(), this.min, this.max, step));
	}

	public int min() {
		return this.min;
	}

	public int max() {
		return this.max;
	}

	public int step() {
		return this.step;
	}

	@Override
	public IntRange value() {
		return this.value;
	}

	@Override
	public IntRange defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(IntRange value) {
		set(value);
	}

	@Override
	public Codec<IntRange> codec() {
		return IntRange.CODEC;
	}
}
