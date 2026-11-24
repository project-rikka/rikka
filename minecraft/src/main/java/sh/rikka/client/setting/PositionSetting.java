package sh.rikka.client.setting;

import com.mojang.serialization.Codec;

public final class PositionSetting extends Setting<ScreenPosition, PositionSetting> {
	private final ScreenPosition defaultValue;
	private volatile ScreenPosition value;

	PositionSetting(String id, String translationKey, ScreenPosition defaultValue) {
		super(id, translationKey);
		this.defaultValue = defaultValue;
		this.value = defaultValue;
	}

	public ScreenPosition get() {
		return this.value;
	}

	public void set(ScreenPosition value) {
		if (this.value.equals(value)) {
			return;
		}
		this.value = value;
		changed(value);
	}

	@Override
	public ScreenPosition value() {
		return this.value;
	}

	@Override
	public ScreenPosition defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(ScreenPosition value) {
		set(value);
	}

	@Override
	public Codec<ScreenPosition> codec() {
		return ScreenPosition.CODEC;
	}
}
