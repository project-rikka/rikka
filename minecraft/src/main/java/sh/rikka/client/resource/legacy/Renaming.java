package sh.rikka.client.resource.legacy;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.function.Function;

final class Renaming {
	static final String MINECRAFT_NAMESPACE = "minecraft:";
	static final String MODELS = Assets.MINECRAFT + "models/";
	static final String ITEM_MODELS = MODELS + "item/";
	static final String BLOCKSTATES = Assets.MINECRAFT + "blockstates/";
	static final String ITEMS = Assets.MINECRAFT + "items/";
	static final String JSON = ".json";

	private Renaming() {
	}

	static String unqualified(String reference) {
		return reference.startsWith(MINECRAFT_NAMESPACE) ? reference.substring(MINECRAFT_NAMESPACE.length()) : reference;
	}

	static String qualified(String reference) {
		return reference.indexOf(':') >= 0 ? reference : MINECRAFT_NAMESPACE + reference;
	}

	static void textures(Conversion conversion, Map<String, List<String>> renames) {
		if (renames.isEmpty()) {
			return;
		}
		moveTextures(conversion.assets(), renames);
		rewriteTextureReferences(conversion, first(renames));
	}

	static void texturePrefix(Conversion conversion, String from, String to) {
		Map<String, List<String>> renames = new LinkedHashMap<>();
		for (String path : conversion.assets().paths(Assets.TEXTURES + from)) {
			if (path.endsWith(Assets.PNG)) {
				String id = Assets.id(path, Assets.TEXTURES, Assets.PNG);
				renames.put(id, List.of(to + id.substring(from.length())));
			} else if (path.endsWith(Assets.PNG + Assets.MCMETA)) {
				String id = Assets.id(path, Assets.TEXTURES, Assets.PNG + Assets.MCMETA);
				renames.putIfAbsent(id, List.of(to + id.substring(from.length())));
			}
		}
		moveTextures(conversion.assets(), renames);
		rewriteTextureReferences(conversion, id -> Optional.of(id.startsWith(from) ? to + id.substring(from.length()) : id));
	}

	static void moveTree(Conversion conversion, String from, String to) {
		Assets assets = conversion.assets();
		for (String path : assets.paths(from)) {
			String target = to + path.substring(from.length());
			if (!assets.has(target)) {
				assets.move(path, target);
			} else {
				assets.remove(path);
			}
		}
	}

	static void moveFiles(Conversion conversion, String from, String to) {
		Assets assets = conversion.assets();
		for (String path : assets.paths(from)) {
			if (path.indexOf('/', from.length()) < 0 && !assets.has(to + path.substring(from.length()))) {
				assets.move(path, to + path.substring(from.length()));
			}
		}
	}

	static void models(Conversion conversion, Map<String, List<String>> renames) {
		if (renames.isEmpty()) {
			return;
		}
		Documents documents = conversion.documents();
		Map<String, JsonElement> moved = new LinkedHashMap<>();
		for (String from : renames.keySet()) {
			documents.remove(modelPath(from)).ifPresent(json -> moved.put(from, json));
		}
		for (Map.Entry<String, List<String>> rename : renames.entrySet()) {
			JsonElement json = moved.get(rename.getKey());
			if (json == null) {
				continue;
			}
			for (String to : rename.getValue()) {
				documents.put(modelPath(to), json.deepCopy());
			}
		}
		rewriteModelReferences(conversion, first(renames));
	}

	static void rewriteTextureReferences(Conversion conversion, Function<String, Optional<String>> mapping) {
		Documents documents = conversion.documents();
		for (String path : documents.paths(Renaming::isModel)) {
			JsonElement json = documents.get(path).orElseThrow();
			if (!json.isJsonObject() || !(json.getAsJsonObject().get("textures") instanceof JsonObject textures)) {
				continue;
			}
			for (Map.Entry<String, JsonElement> entry : textures.entrySet()) {
				if (entry.getValue() instanceof JsonPrimitive primitive && primitive.isString()) {
					remapReference(primitive.getAsString(), mapping).ifPresent(entry::setValue);
				}
			}
		}
		for (String path : documents.paths(Assets.MINECRAFT + "atlases/")) {
			JsonElement json = documents.get(path).orElseThrow();
			if (json.isJsonObject() && json.getAsJsonObject().get("sources") instanceof JsonArray sources) {
				for (JsonElement source : sources) {
					if (source instanceof JsonObject object && object.get("resource") instanceof JsonPrimitive resource && resource.isString()) {
						remapReference(resource.getAsString(), mapping).ifPresent(value -> object.add("resource", value));
					}
				}
			}
		}
	}

	static void rewriteModelReferences(Conversion conversion, Function<String, Optional<String>> mapping) {
		Documents documents = conversion.documents();
		for (String path : documents.paths(Renaming::isModel)) {
			if (!(documents.get(path).orElseThrow() instanceof JsonObject model)) {
				continue;
			}
			if (model.get("parent") instanceof JsonPrimitive parent && parent.isString()) {
				remapReference(parent.getAsString(), mapping).ifPresent(value -> model.add("parent", value));
			}
			if (model.get("overrides") instanceof JsonArray overrides) {
				rewriteModelKeys(overrides, mapping);
			}
		}
		rewriteBlockstateModels(conversion, mapping);
		for (String path : documents.paths(ITEMS)) {
			documents.get(path).ifPresent(json -> rewriteModelKeys(json, mapping));
		}
	}

	static void rewriteBlockstateModels(Conversion conversion, Function<String, Optional<String>> mapping) {
		Documents documents = conversion.documents();
		for (String path : documents.paths(Renaming::isBlockstate)) {
			documents.get(path).ifPresent(json -> rewriteModelKeys(json, mapping));
		}
	}

	static boolean isModel(String path) {
		return path.startsWith(Assets.ROOT) && path.contains("/models/") && path.endsWith(JSON);
	}

	static boolean isBlockstate(String path) {
		return path.startsWith(Assets.ROOT) && path.contains("/blockstates/") && path.endsWith(JSON);
	}

	private static String modelPath(String id) {
		return MODELS + id + JSON;
	}

	private static Function<String, Optional<String>> first(Map<String, List<String>> renames) {
		return id -> {
			List<String> targets = renames.get(id);
			return targets == null ? Optional.of(id) : targets.stream().findFirst();
		};
	}

	private static void moveTextures(Assets assets, Map<String, List<String>> renames) {
		Map<String, Asset> images = new LinkedHashMap<>();
		Map<String, Asset> metas = new LinkedHashMap<>();
		for (String from : renames.keySet()) {
			assets.remove(Assets.texture(from)).ifPresent(asset -> images.put(from, asset));
			assets.remove(Assets.textureMeta(from)).ifPresent(asset -> metas.put(from, asset));
		}
		for (Map.Entry<String, List<String>> rename : renames.entrySet()) {
			Asset image = images.get(rename.getKey());
			Asset meta = metas.get(rename.getKey());
			for (String to : rename.getValue()) {
				if (image != null) {
					assets.put(Assets.texture(to), image);
				}
				if (meta != null) {
					assets.put(Assets.textureMeta(to), meta);
				}
			}
		}
	}

	private static void rewriteModelKeys(JsonElement json, Function<String, Optional<String>> mapping) {
		if (json instanceof JsonObject object) {
			for (Map.Entry<String, JsonElement> entry : new ArrayList<>(object.entrySet())) {
				if (entry.getKey().equals("model") && entry.getValue() instanceof JsonPrimitive primitive && primitive.isString()) {
					remapReference(primitive.getAsString(), mapping).ifPresent(value -> object.add("model", value));
				} else {
					rewriteModelKeys(entry.getValue(), mapping);
				}
			}
		} else if (json instanceof JsonArray array) {
			for (JsonElement element : array) {
				rewriteModelKeys(element, mapping);
			}
		}
	}

	private static Optional<JsonPrimitive> remapReference(String reference, Function<String, Optional<String>> mapping) {
		if (reference.startsWith("#")) {
			return Optional.empty();
		}
		boolean namespaced = reference.startsWith(MINECRAFT_NAMESPACE);
		if (!namespaced && reference.indexOf(':') >= 0) {
			return Optional.empty();
		}
		String id = unqualified(reference);
		Optional<String> mapped = mapping.apply(id);
		if (mapped.isEmpty() || mapped.get().equals(id)) {
			return Optional.empty();
		}
		return Optional.of(new JsonPrimitive(namespaced ? MINECRAFT_NAMESPACE + mapped.get() : mapped.get()));
	}
}
