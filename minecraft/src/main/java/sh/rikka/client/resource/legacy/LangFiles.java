package sh.rikka.client.resource.legacy;

import com.google.gson.JsonObject;
import java.io.IOException;
import java.util.Locale;
import java.util.Map;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

final class LangFiles implements Upgrade {
	private static final Logger LOGGER = LoggerFactory.getLogger(LangFiles.class);
	private static final String LEGACY_EXTENSION = ".lang";

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		for (String path : assets.paths(Assets.ROOT)) {
			if (!path.endsWith(LEGACY_EXTENSION) || !path.contains("/lang/")) {
				continue;
			}
			Asset legacy = assets.remove(path).orElseThrow();
			String target = path.substring(0, path.length() - LEGACY_EXTENSION.length()).toLowerCase(Locale.ROOT) + Renaming.JSON;
			try {
				assets.putJson(target, convert(Text.decode(legacy.bytes())));
			} catch (IOException failure) {
				LOGGER.warn("dropped unreadable legacy language file {}: {}", path, failure.getMessage());
			}
		}
	}

	private static JsonObject convert(String text) {
		Map<String, String> renames = LegacyData.langKeyRenames();
		JsonObject json = new JsonObject();
		for (String line : text.split("\\R")) {
			int separator = line.indexOf('=');
			if (line.startsWith("#") || separator <= 0) {
				continue;
			}
			String key = line.substring(0, separator).trim();
			json.addProperty(renames.getOrDefault(key, key), line.substring(separator + 1));
		}
		return json;
	}
}
