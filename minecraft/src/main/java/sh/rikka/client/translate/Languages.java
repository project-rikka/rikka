package sh.rikka.client.translate;

import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

public final class Languages {
	public static final String FALLBACK = "en";

	private static final List<String> TARGETS = List.of(
		"aa", "ab", "ace", "ach", "af", "ak", "alz", "am", "ar", "as", "av", "awa", "ay", "az", "ba",
		"bal", "ban", "bbc", "bci", "be", "bem", "ber", "ber-Latn", "bew", "bg", "bho", "bik", "bm",
		"bm-Nkoo", "bn", "bo", "br", "bs", "bts", "btx", "bua", "ca", "ce", "ceb", "cgg", "ch", "chk",
		"chm", "ckb", "cnh", "co", "crh", "crh-Latn", "crs", "cs", "cv", "cy", "da", "de", "din",
		"doi", "dov", "dv", "dyu", "dz", "ee", "el", "en", "eo", "es", "es-MX", "et", "eu", "fa", "ff",
		"fi", "fil", "fj", "fo", "fon", "fr", "fr-CA", "fur", "fy", "ga", "gaa", "gd", "gl", "gn",
		"gom", "gu", "gv", "ha", "haw", "he", "hi", "hil", "hmn", "hr", "hrx", "ht", "hu", "hy", "iba",
		"id", "ig", "ilo", "is", "it", "iu", "iu-Latn", "ja", "jam", "jv", "ka", "kac", "kek", "kg",
		"kha", "kk", "kl", "km", "kn", "ko", "kr", "kri", "ktu", "ku", "kv", "ky", "la", "lb", "lg",
		"li", "lij", "lmo", "ln", "lo", "lt", "ltg", "lua", "luo", "lus", "lv", "mad", "mai", "mak",
		"mam", "mfe", "mg", "mh", "mi", "min", "mk", "ml", "mn", "mni-Mtei", "mr", "ms", "ms-Arab",
		"mt", "mwr", "my", "nb", "ndc-ZW", "ne", "new", "nhe", "nl", "nr", "nso", "nus", "ny", "oc",
		"om", "or", "os", "pa", "pa-Arab", "pag", "pam", "pap", "pl", "prs", "ps", "pt", "pt-PT", "qu",
		"rn", "ro", "rom", "ru", "rw", "sa", "sah", "sat", "sat-Latn", "scn", "sd", "se", "sg", "shn",
		"si", "sk", "sl", "sm", "sn", "so", "sq", "sr", "ss", "st", "su", "sus", "sv", "sw", "szl",
		"ta", "tcy", "te", "tet", "tg", "th", "ti", "tiv", "tk", "tn", "to", "tpi", "tr", "trp", "ts",
		"tt", "tum", "ty", "tyv", "udm", "ug", "uk", "ur", "uz", "ve", "vec", "vi", "war", "wo", "xh",
		"yi", "yo", "yua", "yue", "zap", "zh-Hans", "zh-Hant", "zu"
	);
	private static final Set<String> TARGET_SET = Set.copyOf(TARGETS);
	private static final Map<String, String> SERVICE_CODES = Map.of(
		"zh-Hans", "zh-CN",
		"zh-Hant", "zh-TW",
		"fil", "tl",
		"nb", "no",
		"prs", "fa-AF"
	);
	private static final Map<String, String> SERVICE_TAGS = Map.of(
		"zh-CN", "zh-Hans",
		"zh-SG", "zh-Hans",
		"zh-TW", "zh-Hant",
		"zh-HK", "zh-Hant",
		"tl", "fil",
		"no", "nb",
		"fa-AF", "prs",
		"iw", "he",
		"jw", "jv"
	);
	private static final Map<String, String> UNNAMED = Map.ofEntries(
		Map.entry("alz", "Alur"),
		Map.entry("bci", "Baoulé"),
		Map.entry("bts", "Batak Simalungun"),
		Map.entry("btx", "Batak Karo"),
		Map.entry("cnh", "Hakha Chin"),
		Map.entry("dov", "Dombe"),
		Map.entry("gom", "Konkani"),
		Map.entry("hrx", "Hunsrik"),
		Map.entry("kek", "Q'eqchi'"),
		Map.entry("ktu", "Kituba"),
		Map.entry("mam", "Mam"),
		Map.entry("ndc-ZW", "Ndau"),
		Map.entry("nhe", "Eastern Huasteca Nahuatl"),
		Map.entry("prs", "Dari"),
		Map.entry("trp", "Kokborok"),
		Map.entry("yua", "Yucatec Maya")
	);
	private static final Map<String, String> GAME_LANGUAGES = Map.ofEntries(
		Map.entry("zh_cn", "zh-Hans"),
		Map.entry("zh_sg", "zh-Hans"),
		Map.entry("zh_tw", "zh-Hant"),
		Map.entry("zh_hk", "zh-Hant"),
		Map.entry("zh_mo", "zh-Hant"),
		Map.entry("lzh", "zh-Hant"),
		Map.entry("pt_pt", "pt-PT"),
		Map.entry("es_mx", "es-MX"),
		Map.entry("fr_ca", "fr-CA"),
		Map.entry("tl_ph", "fil"),
		Map.entry("nn_no", "nb"),
		Map.entry("no_no", "nb")
	);

	private Languages() {
	}

	public static List<String> targets() {
		return TARGETS;
	}

	public static boolean isTarget(String tag) {
		return TARGET_SET.contains(tag);
	}

	public static String serviceCode(String tag) {
		return SERVICE_CODES.getOrDefault(tag, tag);
	}

	public static String fromServiceCode(String code) {
		return SERVICE_TAGS.getOrDefault(code, code);
	}

	public static String forGame(String code) {
		String lower = code.toLowerCase(Locale.ROOT);
		String mapped = GAME_LANGUAGES.get(lower);
		if (mapped != null) {
			return mapped;
		}
		int separator = lower.indexOf('_');
		String language = separator < 0 ? lower : lower.substring(0, separator);
		return isTarget(language) ? language : FALLBACK;
	}

	public static Locale locale(String gameCode) {
		return Locale.forLanguageTag(gameCode.replace('_', '-'));
	}

	public static String name(String tag, Locale display) {
		Locale locale = Locale.forLanguageTag(tag);
		String name = locale.getDisplayName(display);
		String code = locale.getLanguage();
		if (name.isEmpty() || name.equals(code) || name.startsWith(code + " (")) {
			return UNNAMED.getOrDefault(tag, tag);
		}
		return capitalized(name, display);
	}

	public static String nativeName(String tag) {
		Locale locale = Locale.forLanguageTag(tag);
		return name(tag, locale);
	}

	public static boolean sameLanguage(String detected, String target) {
		if (detected.equals(target)) {
			return true;
		}
		String language = primary(detected);
		return !language.equals("zh") && language.equals(primary(target));
	}

	private static String primary(String tag) {
		int separator = tag.indexOf('-');
		return separator < 0 ? tag : tag.substring(0, separator);
	}

	private static String capitalized(String name, Locale display) {
		int first = name.codePointAt(0);
		if (!Character.isLowerCase(first)) {
			return name;
		}
		return name.substring(0, Character.charCount(first)).toUpperCase(display) + name.substring(Character.charCount(first));
	}
}
