package sh.rikka.client.resource.legacy;

final class RedstoneDust implements Upgrade {
	private static final String LINE = "blocks/redstone_dust_line";
	private static final String CROSS = "blocks/redstone_dust_cross";
	private static final int TILE = 16;
	private static final Rect DOT = Rect.of(4, 4, 8, 8);

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		assets.findTexture(LINE).ifPresent(line -> {
			assets.removeTexture(LINE);
			assets.putTexture(LINE + "0", () -> line.raster(assets.decoder()).rotatedClockwise());
			assets.putTexture(LINE + "1", () -> line.raster(assets.decoder()).rotated(3));
		});
		assets.findTexture(CROSS).ifPresent(cross -> {
			assets.removeTexture(CROSS);
			assets.putTexture("blocks/redstone_dust_dot", () -> {
				Raster source = cross.raster(assets.decoder());
				Rect dot = DOT.scaled(source.scale(TILE));
				Raster result = Raster.blank(source.width(), source.height());
				result.draw(source, dot, dot.x(), dot.y());
				return result;
			});
		});
	}
}
