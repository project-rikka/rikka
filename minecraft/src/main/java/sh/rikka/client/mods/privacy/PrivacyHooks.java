package sh.rikka.client.mods.privacy;

import com.mojang.authlib.GameProfile;
import net.minecraft.network.chat.Style;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.util.StringDecomposer;
import net.minecraft.world.entity.player.PlayerSkin;

public final class PrivacyHooks {
	private static final ThreadLocal<NameFilter> FILTERS = ThreadLocal.withInitial(NameFilter::new);

	private PrivacyHooks() {
	}

	public static boolean protectsText() {
		StreamerModeMod streamerMode = StreamerModeMod.active();
		return streamerMode != null && !streamerMode.names().isEmpty();
	}

	public static FormattedCharSequence text(FormattedCharSequence text) {
		StreamerModeMod streamerMode = StreamerModeMod.active();
		if (streamerMode == null) {
			return text;
		}
		NameTable names = streamerMode.names();
		if (names.isEmpty()) {
			return text;
		}
		return output -> {
			NameFilter shared = FILTERS.get();
			NameFilter filter = shared.isOpen() ? new NameFilter() : shared;
			filter.open(output, names);
			try {
				return text.accept(filter) && filter.finish();
			} finally {
				filter.close();
			}
		};
	}

	public static String text(String text) {
		if (!protectsText()) {
			return text;
		}
		StringBuilder shown = new StringBuilder(text.length());
		text(output -> StringDecomposer.iterateFormatted(text, Style.EMPTY, output)).accept((position, style, codepoint) -> {
			shown.appendCodePoint(codepoint);
			return true;
		});
		return shown.toString();
	}

	public static PlayerSkin skin(GameProfile profile, PlayerSkin vanilla) {
		StreamerModeMod streamerMode = StreamerModeMod.active();
		return streamerMode == null ? vanilla : streamerMode.skin(profile, vanilla);
	}
}
