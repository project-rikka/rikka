package sh.rikka.client.resource.legacy;

import java.io.IOException;
import java.util.List;
import java.util.Optional;

record GuiStyle(GuiPalette palette, Raster slot, Raster tile, double scale) {
	static final int BACKGROUND = 0xFFC6C6C6;
	static final int SLOT_FILL = 0xFF8B8B8B;
	static final int SLOT_SHADOW = 0xFF373737;
	static final int SLOT_LIGHT = 0xFFFFFFFF;
	static final int SLOT_SIZE = 18;
	static final String CONTAINERS = "gui/container/";
	static final String SPRITES = "gui/sprites/";
	static final String CONTAINER_SPRITES = SPRITES + "container/";
	static final String SLOT_SPRITES = CONTAINER_SPRITES + "slot/";

	private static final int MAX_TILE = 16;
	private static final int MIN_TILE = 4;
	private static final List<Source> SOURCES = List.of(
		new Source("generic_54", Rect.of(7, 17, SLOT_SIZE, SLOT_SIZE)),
		new Source("crafting_table", Rect.of(7, 83, SLOT_SIZE, SLOT_SIZE)),
		new Source("dispenser", Rect.of(7, 83, SLOT_SIZE, SLOT_SIZE)),
		new Source("hopper", Rect.of(7, 50, SLOT_SIZE, SLOT_SIZE)),
		new Source("furnace", Rect.of(7, 83, SLOT_SIZE, SLOT_SIZE)),
		new Source("inventory", Rect.of(7, 83, SLOT_SIZE, SLOT_SIZE))
	);

	public static Optional<GuiStyle> learn(Conversion conversion) throws IOException {
		Assets assets = conversion.assets();
		for (Source source : SOURCES) {
			String id = CONTAINERS + source.container();
			if (!assets.hasTexture(id)) {
				continue;
			}
			Optional<Raster> vanilla = conversion.vanilla().texture(id);
			if (vanilla.isEmpty()) {
				continue;
			}
			Raster pack = assets.textureRaster(id);
			double scale = pack.scale(vanilla.get().width());
			Rect tileRect = backgroundSquare(vanilla.get());
			return Optional.of(new GuiStyle(
				GuiPalette.learn(pack, vanilla.get()),
				pack.crop(source.slot().scaled(scale)),
				pack.crop(tileRect.scaled(scale)),
				scale
			));
		}
		return Optional.empty();
	}

	public int scaled(int vanillaUnits) {
		return (int) Math.round(vanillaUnits * this.scale);
	}

	private static Rect backgroundSquare(Raster vanilla) {
		int width = vanilla.width();
		int height = vanilla.height();
		int[] sizes = new int[width * height];
		Rect best = Rect.of(0, 0, 0, 0);
		for (int y = 0; y < height; y++) {
			for (int x = 0; x < width; x++) {
				if (vanilla.get(x, y) != BACKGROUND) {
					continue;
				}
				int size = 1;
				if (x > 0 && y > 0) {
					size = 1 + Math.min(sizes[(y - 1) * width + x - 1], Math.min(sizes[(y - 1) * width + x], sizes[y * width + x - 1]));
				}
				size = Math.min(size, MAX_TILE);
				sizes[y * width + x] = size;
				if (size > best.width()) {
					best = Rect.of(x - size + 1, y - size + 1, size, size);
				}
			}
		}
		if (best.width() < MIN_TILE) {
			return Rect.of(4, 4, MIN_TILE, MIN_TILE);
		}
		return best;
	}

	private record Source(String container, Rect slot) {
	}
}
