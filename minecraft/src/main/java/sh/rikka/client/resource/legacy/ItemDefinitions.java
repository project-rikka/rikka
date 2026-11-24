package sh.rikka.client.resource.legacy;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.io.IOException;
import java.util.Optional;

final class ItemDefinitions implements Upgrade {
	private static final String MODEL = "model";
	private static final String MODEL_TYPE = "minecraft:model";
	private static final String CUSTOM_MODEL_DATA = "custom_model_data";
	private static final String BUILTIN = "builtin/";

	@Override
	public void apply(Conversion conversion) throws IOException {
		Documents documents = conversion.documents();
		for (String path : documents.paths(Renaming.ITEM_MODELS)) {
			String name = Assets.id(path, Renaming.ITEM_MODELS, Renaming.JSON);
			String definitionPath = Renaming.ITEMS + name + Renaming.JSON;
			if (documents.has(definitionPath) || !(documents.get(path).orElseThrow() instanceof JsonObject model) || isBuiltin(model)) {
				continue;
			}
			Optional<JsonElement> vanilla = conversion.vanilla().json(definitionPath);
			if (vanilla.isEmpty() || !(vanilla.get() instanceof JsonObject vanillaDefinition)) {
				continue;
			}
			String own = Renaming.MINECRAFT_NAMESPACE + "item/" + name;
			Optional<JsonArray> customModelData = customModelData(model.remove("overrides"));
			boolean custom = !referencesModel(vanillaDefinition, own) && !isPlainParent(model);
			if (!custom && customModelData.isEmpty() || !(vanillaDefinition.get(MODEL) instanceof JsonObject vanillaModel)) {
				continue;
			}
			JsonObject fallback;
			if (custom) {
				fallback = plainModel(own);
				if (vanillaModel.has("tints")) {
					fallback.add("tints", vanillaModel.get("tints").deepCopy());
				}
			} else {
				fallback = vanillaModel.deepCopy();
			}
			JsonObject definition = new JsonObject();
			definition.add(MODEL, customModelData.map(entries -> rangeDispatch(entries, fallback)).orElse(fallback));
			documents.put(definitionPath, definition);
		}
	}

	private static boolean isBuiltin(JsonObject model) {
		return model.get("parent") instanceof JsonPrimitive parent && Renaming.unqualified(parent.getAsString()).startsWith(BUILTIN);
	}

	private static boolean referencesModel(JsonObject definition, String model) {
		return definition.toString().contains("\"" + model + "\"");
	}

	private static boolean isPlainParent(JsonObject model) {
		return model.size() == 1 && model.get("parent") instanceof JsonPrimitive parent && Renaming.unqualified(parent.getAsString()).startsWith("block/");
	}

	private static JsonObject plainModel(String model) {
		JsonObject result = new JsonObject();
		result.addProperty("type", MODEL_TYPE);
		result.addProperty(MODEL, model);
		return result;
	}

	private static Optional<JsonArray> customModelData(JsonElement overrides) {
		if (!(overrides instanceof JsonArray list) || list.isEmpty()) {
			return Optional.empty();
		}
		JsonArray entries = new JsonArray();
		for (JsonElement element : list) {
			if (!(element instanceof JsonObject override)
				|| !(override.get("predicate") instanceof JsonObject predicate)
				|| predicate.size() != 1
				|| !(predicate.get(CUSTOM_MODEL_DATA) instanceof JsonPrimitive threshold)
				|| !threshold.isNumber()
				|| !(override.get(MODEL) instanceof JsonPrimitive target)) {
				return Optional.empty();
			}
			JsonObject entry = new JsonObject();
			entry.addProperty("threshold", threshold.getAsDouble());
			entry.add(MODEL, plainModel(Renaming.qualified(target.getAsString())));
			entries.add(entry);
		}
		return Optional.of(entries);
	}

	private static JsonObject rangeDispatch(JsonArray entries, JsonObject fallback) {
		JsonObject dispatch = new JsonObject();
		dispatch.addProperty("type", "minecraft:range_dispatch");
		dispatch.addProperty("property", "minecraft:custom_model_data");
		dispatch.add("entries", entries);
		dispatch.add("fallback", fallback);
		return dispatch;
	}
}
