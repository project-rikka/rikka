package sh.rikka.client.resource.legacy;

final class SweepFrames implements Upgrade {
	private static final String SHEET = "entity/sweep";
	private static final int REFERENCE_WIDTH = 128;
	private static final int COLUMNS = 4;
	private static final int FRAMES = 8;
	private static final int FRAME_WIDTH = 32;
	private static final int FRAME_HEIGHT = 16;

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		assets.findTexture(SHEET).ifPresent(sheet -> {
			assets.removeTexture(SHEET);
			for (int frame = 0; frame < FRAMES; frame++) {
				Rect cell = Rect.of(frame % COLUMNS * FRAME_WIDTH, frame / COLUMNS * FRAME_HEIGHT, FRAME_WIDTH, FRAME_HEIGHT);
				assets.putTexture("particle/sweep_" + frame, () -> {
					Raster source = sheet.raster(assets.decoder());
					Rect rect = cell.scaled(source.scale(REFERENCE_WIDTH));
					return source.crop(rect).resized(rect.width(), rect.width());
				});
			}
		});
	}
}
