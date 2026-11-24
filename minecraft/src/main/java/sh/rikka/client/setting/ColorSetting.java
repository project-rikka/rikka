package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import net.minecraft.util.ARGB;
import net.minecraft.util.Mth;
import net.minecraft.util.Util;

public final class ColorSetting extends Setting<ColorValue, ColorSetting> {
	private static final long CHROMA_PERIOD_MILLIS = 4000;

	private final ColorValue defaultValue;
	private boolean opaque;
	private volatile ColorValue value;

	ColorSetting(String id, String translationKey, int defaultArgb) {
		super(id, translationKey);
		this.defaultValue = new ColorValue(defaultArgb, false);
		this.value = this.defaultValue;
	}

	public ColorSetting opaque() {
		if (ARGB.alpha(this.defaultValue.argb()) != 0xFF) {
			throw new IllegalArgumentException("default color of opaque " + id() + " is translucent");
		}
		this.opaque = true;
		return this;
	}

	public boolean isOpaque() {
		return this.opaque;
	}

	public ColorValue get() {
		return this.value;
	}

	public int argb() {
		ColorValue current = this.value;
		if (!current.chroma()) {
			return current.argb();
		}
		float hue = Util.getMillis() % CHROMA_PERIOD_MILLIS / (float) CHROMA_PERIOD_MILLIS;
		return Mth.hsvToArgb(hue, 1.0F, 1.0F, ARGB.alpha(current.argb()));
	}

	public void set(ColorValue value) {
		ColorValue normalized = this.opaque ? value.withArgb(ARGB.opaque(value.argb())) : value;
		if (this.value.equals(normalized)) {
			return;
		}
		this.value = normalized;
		changed(normalized);
	}

	public void set(int argb) {
		set(this.value.withArgb(argb));
	}

	public void setChroma(boolean chroma) {
		set(this.value.withChroma(chroma));
	}

	@Override
	public ColorValue value() {
		return this.value;
	}

	@Override
	public ColorValue defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(ColorValue value) {
		set(value);
	}

	@Override
	public Codec<ColorValue> codec() {
		return ColorValue.CODEC;
	}
}
