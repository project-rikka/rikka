package sh.rikka.client.setting;

import com.mojang.serialization.Codec;

public final class BoolSetting extends Setting<Boolean, BoolSetting> {
	private final boolean defaultValue;
	private volatile boolean value;

	BoolSetting(String id, String translationKey, boolean defaultValue) {
		super(id, translationKey);
		this.defaultValue = defaultValue;
		this.value = defaultValue;
	}

	public boolean get() {
		return this.value;
	}

	public void set(boolean value) {
		if (this.value == value) {
			return;
		}
		this.value = value;
		changed(value);
	}

	public void toggle() {
		set(!this.value);
	}

	@Override
	public Boolean value() {
		return this.value;
	}

	@Override
	public Boolean defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(Boolean value) {
		set(value);
	}

	@Override
	public Codec<Boolean> codec() {
		return Codec.BOOL;
	}
}
