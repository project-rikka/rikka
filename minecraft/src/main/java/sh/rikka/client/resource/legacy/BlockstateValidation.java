package sh.rikka.client.resource.legacy;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.util.Arrays;
import java.util.Optional;
import java.util.Set;
import java.util.TreeSet;
import java.util.stream.Collectors;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

final class BlockstateValidation implements Upgrade {
	private static final Logger LOGGER = LoggerFactory.getLogger(BlockstateValidation.class);
	private static final String VARIANTS = "variants";
	private static final String MULTIPART = "multipart";

	@Override
	public void apply(Conversion conversion) throws IOException {
		Documents documents = conversion.documents();
		for (String path : documents.paths(Renaming.BLOCKSTATES)) {
			JsonElement pack = documents.get(path).orElseThrow();
			Optional<JsonElement> vanilla = conversion.vanilla().json(path);
			if (vanilla.isEmpty() || !compatible(pack, vanilla.get())) {
				documents.remove(path);
				LOGGER.debug("dropped legacy blockstate {} that no longer matches the block's states", path);
			}
		}
	}

	private static boolean compatible(JsonElement pack, JsonElement vanilla) {
		if (!(pack instanceof JsonObject packObject) || !(vanilla instanceof JsonObject vanillaObject)) {
			return false;
		}
		if (packObject.get(VARIANTS) instanceof JsonObject packVariants) {
			Set<String> vanillaKeys = variantKeys(vanillaObject.get(VARIANTS));
			return vanillaKeys != null && variantKeys(packVariants).equals(vanillaKeys);
		}
		if (packObject.get(MULTIPART) instanceof JsonArray packParts && vanillaObject.get(MULTIPART) instanceof JsonArray vanillaParts) {
			return properties(vanillaParts).containsAll(properties(packParts));
		}
		return false;
	}

	private static Set<String> variantKeys(JsonElement variants) {
		if (!(variants instanceof JsonObject object)) {
			return null;
		}
		Set<String> keys = new TreeSet<>();
		for (String key : object.keySet()) {
			keys.add(Arrays.stream(key.split(",")).map(String::trim).filter(part -> !part.isEmpty()).sorted().collect(Collectors.joining(",")));
		}
		return keys;
	}

	private static Set<String> properties(JsonArray parts) {
		Set<String> names = new TreeSet<>();
		for (JsonElement part : parts) {
			if (part instanceof JsonObject object && object.get("when") instanceof JsonObject when) {
				collect(when, names);
			}
		}
		return names;
	}

	private static void collect(JsonObject when, Set<String> names) {
		for (var entry : when.entrySet()) {
			if ((entry.getKey().equals("OR") || entry.getKey().equals("AND")) && entry.getValue() instanceof JsonArray terms) {
				for (JsonElement term : terms) {
					if (term instanceof JsonObject object) {
						collect(object, names);
					}
				}
			} else {
				names.add(entry.getKey());
			}
		}
	}
}
