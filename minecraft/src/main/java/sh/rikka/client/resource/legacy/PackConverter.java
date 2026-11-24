package sh.rikka.client.resource.legacy;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class PackConverter {
	private static final Logger LOGGER = LoggerFactory.getLogger(PackConverter.class);
	private static final String PACK = "pack";
	private static final String LANGUAGE = "language";
	private static final List<String> DOCUMENT_ROOTS = List.of(Renaming.ITEMS, Assets.MINECRAFT + "atlases/");

	private PackConverter() {
	}

	public static Assets convert(PackArchive archive, PackDescriptor descriptor, Vanilla vanilla, ImageDecoder decoder) {
		Assets assets = new Assets(decoder);
		for (Map.Entry<String, String> entry : assetEntries(archive, descriptor).entrySet()) {
			assets.put(entry.getKey(), Asset.stored(archive, entry.getValue()));
		}
		Documents documents = Documents.take(assets, PackConverter::isDocument);
		Conversion conversion = new Conversion(assets, documents, descriptor.format(), vanilla);
		for (Upgrades.Step step : Upgrades.STEPS) {
			if (!conversion.sourceBefore(step.before())) {
				continue;
			}
			try {
				step.upgrade().apply(conversion);
			} catch (IOException | RuntimeException failure) {
				LOGGER.warn("skipped legacy pack upgrade {}", step.upgrade().getClass().getSimpleName(), failure);
			}
		}
		documents.store(assets);
		return assets;
	}

	public static Set<String> namespaces(PackArchive archive, PackDescriptor descriptor) {
		Set<String> namespaces = new TreeSet<>();
		for (String path : assetEntries(archive, descriptor).keySet()) {
			int end = path.indexOf('/', Assets.ROOT.length());
			if (end > Assets.ROOT.length()) {
				namespaces.add(NameNormalization.normalize(path.substring(Assets.ROOT.length(), end)));
			}
		}
		return namespaces;
	}

	public static byte[] metadata(PackDescriptor descriptor, int currentFormat) {
		JsonObject result = new JsonObject();
		for (Map.Entry<String, JsonElement> section : descriptor.json().entrySet()) {
			switch (section.getKey()) {
				case PACK, "overlays" -> {
				}
				case LANGUAGE -> result.add(LANGUAGE, lowercaseKeys(section.getValue()));
				default -> result.add(section.getKey(), section.getValue().deepCopy());
			}
		}
		JsonObject pack = new JsonObject();
		if (descriptor.json().get(PACK) instanceof JsonObject original && original.has("description")) {
			pack.add("description", original.get("description").deepCopy());
		} else {
			pack.addProperty("description", "");
		}
		pack.addProperty("min_format", currentFormat);
		pack.addProperty("max_format", currentFormat);
		result.add(PACK, pack);
		return Text.encode(result);
	}

	private static Map<String, String> assetEntries(PackArchive archive, PackDescriptor descriptor) {
		Map<String, String> entries = new LinkedHashMap<>();
		for (String entry : archive.entries()) {
			if (entry.startsWith(Assets.ROOT)) {
				entries.put(entry, entry);
			}
		}
		for (String overlay : descriptor.overlays()) {
			String prefix = overlay + "/";
			for (String entry : archive.entries()) {
				if (entry.startsWith(prefix + Assets.ROOT)) {
					entries.put(entry.substring(prefix.length()), entry);
				}
			}
		}
		return entries;
	}

	private static JsonElement lowercaseKeys(JsonElement languages) {
		if (!(languages instanceof JsonObject object)) {
			return languages.deepCopy();
		}
		JsonObject result = new JsonObject();
		for (Map.Entry<String, JsonElement> entry : object.entrySet()) {
			result.add(entry.getKey().toLowerCase(Locale.ROOT), entry.getValue().deepCopy());
		}
		return result;
	}

	private static boolean isDocument(String path) {
		return Renaming.isModel(path) || Renaming.isBlockstate(path) || DOCUMENT_ROOTS.stream().anyMatch(path::startsWith);
	}
}
