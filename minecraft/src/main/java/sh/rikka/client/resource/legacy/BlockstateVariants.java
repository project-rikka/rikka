package sh.rikka.client.resource.legacy;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.List;
import java.util.Map;

final class BlockstateVariants implements Upgrade {
	private static final String NORMAL = "normal";
	private static final String BLOCK = "block/";

	@Override
	public void apply(Conversion conversion) {
		Documents documents = conversion.documents();
		Map<String, List<String>> modelRenames = LegacyData.renameStep(PackFormats.V1_13).models();
		for (String path : documents.paths(Renaming.BLOCKSTATES)) {
			JsonElement json = documents.get(path).orElseThrow();
			if (json instanceof JsonObject object && object.get("variants") instanceof JsonObject variants && variants.has(NORMAL)) {
				variants.add("", variants.remove(NORMAL));
			}
			String name = Assets.id(path, Renaming.BLOCKSTATES, Renaming.JSON);
			List<String> targets = modelRenames.get(BLOCK + name);
			if (targets == null || targets.isEmpty()) {
				continue;
			}
			String target = Renaming.BLOCKSTATES + targets.getFirst().substring(BLOCK.length()) + Renaming.JSON;
			if (!documents.has(target)) {
				documents.put(target, documents.remove(path).orElseThrow());
			}
		}
	}
}
