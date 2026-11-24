package sh.rikka.client.resource.legacy;

import java.util.List;

final class WaterTint implements Upgrade {
	private static final int DEFAULT_WATER = 0x3F76E4;
	private static final List<String> TEXTURES = List.of("block/water_still", "block/water_flow", "block/water_overlay");

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		for (String id : TEXTURES) {
			assets.findTexture(id).ifPresent(water -> assets.putTexture(id, () -> water.raster(assets.decoder()).mapped(WaterTint::untint)));
		}
	}

	private static int untint(int argb) {
		double tr = Colors.red(DEFAULT_WATER);
		double tg = Colors.green(DEFAULT_WATER);
		double tb = Colors.blue(DEFAULT_WATER);
		double dot = Colors.red(argb) * tr + Colors.green(argb) * tg + Colors.blue(argb) * tb;
		int gray = (int) Math.round(dot / (tr * tr + tg * tg + tb * tb) * 255.0);
		return Colors.argb(Colors.alpha(argb), gray, gray, gray);
	}
}
