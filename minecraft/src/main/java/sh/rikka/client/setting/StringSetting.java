package sh.rikka.client.setting;

import com.mojang.serialization.Codec;

public final class StringSetting extends Setting<String, StringSetting> {
	private static final int DEFAULT_MAX_LENGTH = 256;

	private final String defaultValue;
	private int maxLength = DEFAULT_MAX_LENGTH;
	private volatile String value;

	StringSetting(String id, String translationKey, String defaultValue) {
		super(id, translationKey);
		requireLength(defaultValue, this.maxLength);
		this.defaultValue = defaultValue;
		this.value = defaultValue;
	}

	public StringSetting maxLength(int maxLength) {
		if (maxLength <= 0) {
			throw new IllegalArgumentException("max length of " + id() + " must be positive");
		}
		requireLength(this.defaultValue, maxLength);
		this.maxLength = maxLength;
		return this;
	}

	private void requireLength(String text, int maxLength) {
		if (text.codePointCount(0, text.length()) > maxLength) {
			throw new IllegalArgumentException("default of " + id() + " is longer than " + maxLength + " characters");
		}
	}

	public int maxLength() {
		return this.maxLength;
	}

	public String get() {
		return this.value;
	}

	public void set(String value) {
		String truncated = value.codePointCount(0, value.length()) > this.maxLength
			? value.substring(0, value.offsetByCodePoints(0, this.maxLength))
			: value;
		if (this.value.equals(truncated)) {
			return;
		}
		this.value = truncated;
		changed(truncated);
	}

	@Override
	public String value() {
		return this.value;
	}

	@Override
	public String defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(String value) {
		set(value);
	}

	@Override
	public Codec<String> codec() {
		return Codec.STRING;
	}
}
