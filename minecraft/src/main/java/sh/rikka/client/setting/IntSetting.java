package sh.rikka.client.setting;

import com.mojang.serialization.Codec;

public final class IntSetting extends Setting<Integer, IntSetting> {
	private final int defaultValue;
	private final int min;
	private final int max;
	private int step = 1;
	private volatile int value;

	IntSetting(String id, String translationKey, int defaultValue, int min, int max) {
		super(id, translationKey);
		Numeric.requireBounds(min, max);
		if (defaultValue < min || defaultValue > max) {
			throw new IllegalArgumentException("default " + defaultValue + " of " + id + " is outside " + min + ".." + max);
		}
		this.defaultValue = defaultValue;
		this.min = min;
		this.max = max;
		this.value = defaultValue;
	}

	public IntSetting step(int step) {
		Numeric.requireStep(step, this.min, this.max);
		if (Numeric.snap(this.defaultValue, this.min, this.max, step) != this.defaultValue) {
			throw new IllegalArgumentException("default " + this.defaultValue + " of " + id() + " is not on step " + step);
		}
		this.step = step;
		return this;
	}

	public int get() {
		return this.value;
	}

	public void set(int value) {
		int snapped = Numeric.snap(value, this.min, this.max, this.step);
		if (this.value == snapped) {
			return;
		}
		this.value = snapped;
		changed(snapped);
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
	public Integer value() {
		return this.value;
	}

	@Override
	public Integer defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(Integer value) {
		set(value);
	}

	@Override
	public Codec<Integer> codec() {
		return Codec.INT;
	}
}
