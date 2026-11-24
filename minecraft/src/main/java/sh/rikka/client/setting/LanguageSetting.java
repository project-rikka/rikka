package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import com.mojang.serialization.DataResult;
import java.util.Locale;
import net.minecraft.client.Minecraft;
import sh.rikka.client.translate.Languages;

public final class LanguageSetting extends Setting<String, LanguageSetting> {
	public static final String GAME = "game";

	private final boolean offersGame;
	private final String defaultValue;
	private final Codec<String> codec;
	private volatile String value;

	LanguageSetting(String id, String translationKey, String defaultValue, boolean offersGame) {
		super(id, translationKey);
		this.offersGame = offersGame;
		if (!accepts(defaultValue)) {
			throw new IllegalArgumentException("default of " + id + " is not an offered language: " + defaultValue);
		}
		this.defaultValue = defaultValue;
		this.value = defaultValue;
		this.codec = Codec.STRING.validate(tag -> accepts(tag) ? DataResult.success(tag) : DataResult.error(() -> "unknown language " + tag));
	}

	public boolean offersGame() {
		return this.offersGame;
	}

	public boolean accepts(String tag) {
		return Languages.isTarget(tag) || (this.offersGame && GAME.equals(tag));
	}

	public String get() {
		return this.value;
	}

	public String resolved() {
		String tag = this.value;
		return GAME.equals(tag) ? gameLanguage() : tag;
	}

	public static String gameLanguage() {
		return Languages.forGame(gameLanguageCode());
	}

	public static Locale gameLocale() {
		return Languages.locale(gameLanguageCode());
	}

	private static String gameLanguageCode() {
		return Minecraft.getInstance().getLanguageManager().getSelected();
	}

	public void set(String tag) {
		if (!accepts(tag)) {
			throw new IllegalArgumentException(id() + " does not offer language " + tag);
		}
		if (this.value.equals(tag)) {
			return;
		}
		this.value = tag;
		changed(tag);
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
		return this.codec;
	}
}
