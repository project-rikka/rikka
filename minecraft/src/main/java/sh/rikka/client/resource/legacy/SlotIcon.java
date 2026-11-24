package sh.rikka.client.resource.legacy;

final class SlotIcon {
	static final int SIZE = 16;

	private static final int TOLERANCE = 3 * 12 * 12;

	private SlotIcon() {
	}

	static Raster extract(Raster withIcon, Raster plain, double scale) {
		int inset = (int) Math.round(scale);
		int size = (int) Math.round(SIZE * scale);
		Raster result = Raster.blank(size, size);
		for (int y = 0; y < size; y++) {
			for (int x = 0; x < size; x++) {
				int sx = x + inset;
				int sy = y + inset;
				if (sx >= withIcon.width() || sy >= withIcon.height() || sx >= plain.width() || sy >= plain.height()) {
					continue;
				}
				int pixel = withIcon.get(sx, sy);
				if (Colors.distance(pixel, plain.get(sx, sy)) > TOLERANCE) {
					result.set(x, y, pixel);
				}
			}
		}
		return result;
	}
}
