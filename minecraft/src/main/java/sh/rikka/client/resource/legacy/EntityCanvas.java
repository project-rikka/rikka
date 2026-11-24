package sh.rikka.client.resource.legacy;

import java.util.Map;

final class EntityCanvas implements Upgrade {
	private static final Map<String, String> SQUARED = Map.of(
		"entity/cow/cow", "entity/cow/temperate_cow",
		"entity/cow/red_mooshroom", "entity/cow/red_mooshroom",
		"entity/cow/brown_mooshroom", "entity/cow/brown_mooshroom",
		"entity/pig/pig", "entity/pig/temperate_pig",
		"entity/pig/pig_saddle", "entity/equipment/pig_saddle/saddle"
	);

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		for (Map.Entry<String, String> entry : SQUARED.entrySet()) {
			assets.findTexture(entry.getKey()).ifPresent(source -> {
				assets.removeTexture(entry.getKey());
				assets.putTexture(entry.getValue(), () -> {
					Raster raster = source.raster(assets.decoder());
					return raster.height() >= raster.width() ? raster : raster.canvas(raster.width(), raster.width());
				});
			});
		}
	}
}
