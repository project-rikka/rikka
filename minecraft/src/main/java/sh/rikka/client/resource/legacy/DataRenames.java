package sh.rikka.client.resource.legacy;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import sh.rikka.client.resource.legacy.LegacyData.RenameStep;

final class DataRenames implements Upgrade {
	private static final String WILDCARD = "*";

	private final RenameStep step;

	private DataRenames(RenameStep step) {
		this.step = step;
	}

	static Upgrade before(int format) {
		return new DataRenames(LegacyData.renameStep(format));
	}

	@Override
	public void apply(Conversion conversion) {
		Renaming.textures(conversion, expand(conversion, this.step.textures()));
		Renaming.models(conversion, this.step.models());
	}

	private static Map<String, List<String>> expand(Conversion conversion, Map<String, List<String>> renames) {
		Map<String, List<String>> expanded = new LinkedHashMap<>();
		for (Map.Entry<String, List<String>> rename : renames.entrySet()) {
			String from = rename.getKey();
			if (!from.endsWith(WILDCARD)) {
				expanded.put(from, rename.getValue());
				continue;
			}
			String prefix = Assets.TEXTURES + from.substring(0, from.length() - WILDCARD.length());
			for (String path : conversion.assets().paths(prefix)) {
				if (path.endsWith(Assets.PNG)) {
					expanded.put(Assets.id(path, Assets.TEXTURES, Assets.PNG), rename.getValue());
				}
			}
		}
		return expanded;
	}
}
