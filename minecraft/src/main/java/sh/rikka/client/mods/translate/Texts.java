package sh.rikka.client.mods.translate;

import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.MutableComponent;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.setting.LanguageSetting;
import sh.rikka.client.translate.Languages;
import sh.rikka.client.translate.TranslationFailure;

final class Texts {
	private static final String PREFIX = RikkaClient.MOD_ID + ".mod." + ChatTranslateMod.ID + ".";

	private Texts() {
	}

	static MutableComponent of(String suffix, Object... arguments) {
		return Component.translatable(PREFIX + suffix, arguments);
	}

	static String languageName(String tag) {
		return Languages.name(tag, LanguageSetting.gameLocale());
	}

	static MutableComponent failure(TranslationFailure failure) {
		return of("error." + failure.reason().id());
	}
}
