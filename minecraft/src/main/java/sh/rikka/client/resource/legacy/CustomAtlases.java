package sh.rikka.client.resource.legacy;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.util.Set;
import java.util.TreeSet;

final class CustomAtlases implements Upgrade {
	private static final String BLOCKS_ATLAS = Assets.MINECRAFT + "atlases/blocks.json";

	@Override
	public void apply(Conversion conversion) {
		Documents documents = conversion.documents();
		if (documents.has(BLOCKS_ATLAS)) {
			return;
		}
		Set<String> resources = new TreeSet<>();
		for (String path : documents.paths(Renaming::isModel)) {
			if (!(documents.get(path).orElseThrow() instanceof JsonObject model) || !(model.get("textures") instanceof JsonObject textures)) {
				continue;
			}
			for (var entry : textures.entrySet()) {
				if (entry.getValue() instanceof JsonPrimitive primitive && primitive.isString()) {
					String reference = primitive.getAsString();
					if (!reference.startsWith("#") && !inDefaultAtlas(reference)) {
						resources.add(reference);
					}
				}
			}
		}
		if (resources.isEmpty()) {
			return;
		}
		JsonArray sources = new JsonArray();
		for (String resource : resources) {
			JsonObject source = new JsonObject();
			source.addProperty("type", "single");
			source.addProperty("resource", resource);
			sources.add(source);
		}
		JsonObject atlas = new JsonObject();
		atlas.add("sources", sources);
		documents.put(BLOCKS_ATLAS, atlas);
	}

	private static boolean inDefaultAtlas(String reference) {
		String id = Renaming.unqualified(reference);
		return id.indexOf(':') < 0 && (id.startsWith("block/") || id.startsWith("item/"));
	}
}
