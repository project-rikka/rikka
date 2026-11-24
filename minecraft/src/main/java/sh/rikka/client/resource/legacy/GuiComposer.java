package sh.rikka.client.resource.legacy;

import java.util.ArrayList;
import java.util.List;

final class GuiComposer {
	private static final int LARGE_SLOT_SIZE = 26;

	private GuiComposer() {
	}

	@FunctionalInterface
	public interface Keep {
		boolean test(int x, int y);
	}

	public static Raster compose(Raster target, Raster reference, Keep keep, GuiStyle style) {
		int width = style.scaled(target.width());
		int height = style.scaled(target.height());
		Raster result = Raster.blank(width, height);
		boolean[] kept = new boolean[target.width() * target.height()];
		for (int y = 0; y < target.height(); y++) {
			for (int x = 0; x < target.width(); x++) {
				int vanilla = target.get(x, y);
				Rect block = Rect.of(x, y, 1, 1).scaled(style.scale());
				if (Colors.alpha(vanilla) == 0) {
					continue;
				}
				if (reference != null && keep.test(x, y) && block.right() <= reference.width() && block.bottom() <= reference.height()) {
					result.draw(reference, block, block.x(), block.y());
					kept[y * target.width() + x] = true;
				} else if (vanilla == GuiStyle.BACKGROUND) {
					tile(result, block, style.tile());
				} else {
					result.fill(block, style.palette().map(vanilla));
				}
			}
		}
		for (Rect slot : slots(target, GuiStyle.SLOT_SIZE)) {
			if (!fullyKept(kept, target.width(), slot)) {
				result.draw(style.slot(), style.scaled(slot.x()), style.scaled(slot.y()));
			}
		}
		for (Rect slot : slots(target, LARGE_SLOT_SIZE)) {
			if (!fullyKept(kept, target.width(), slot)) {
				result.draw(nineSlice(style.slot(), style.scaled(slot.width()), style.scaled(slot.height()), style.scaled(1)), style.scaled(slot.x()), style.scaled(slot.y()));
			}
		}
		return result;
	}

	public static Raster recolor(Raster vanilla, GuiStyle style) {
		Raster upscaled = vanilla.resized(style.scaled(vanilla.width()), style.scaled(vanilla.height()));
		return upscaled.mapped(style.palette()::map);
	}

	private static List<Rect> slots(Raster target, int size) {
		List<Rect> slots = new ArrayList<>();
		for (int y = 0; y + size <= target.height(); y++) {
			for (int x = 0; x + size <= target.width(); x++) {
				if (isSlot(target, x, y, size)) {
					slots.add(Rect.of(x, y, size, size));
				}
			}
		}
		return slots;
	}

	private static boolean isSlot(Raster target, int x, int y, int size) {
		for (int i = 1; i < size - 1; i++) {
			if (target.get(x + i, y) != GuiStyle.SLOT_SHADOW || target.get(x, y + i) != GuiStyle.SLOT_SHADOW) {
				return false;
			}
			if (target.get(x + i, y + size - 1) != GuiStyle.SLOT_LIGHT || target.get(x + size - 1, y + i) != GuiStyle.SLOT_LIGHT) {
				return false;
			}
		}
		for (int j = 1; j < size - 1; j++) {
			for (int i = 1; i < size - 1; i++) {
				if (target.get(x + i, y + j) != GuiStyle.SLOT_FILL) {
					return false;
				}
			}
		}
		return true;
	}

	private static boolean fullyKept(boolean[] kept, int width, Rect rect) {
		for (int y = rect.y(); y < rect.bottom(); y++) {
			for (int x = rect.x(); x < rect.right(); x++) {
				if (!kept[y * width + x]) {
					return false;
				}
			}
		}
		return true;
	}

	private static void tile(Raster result, Rect block, Raster tile) {
		for (int y = block.y(); y < Math.min(block.bottom(), result.height()); y++) {
			for (int x = block.x(); x < Math.min(block.right(), result.width()); x++) {
				result.set(x, y, tile.get(x % tile.width(), y % tile.height()));
			}
		}
	}

	private static Raster nineSlice(Raster source, int width, int height, int border) {
		Raster result = Raster.blank(width, height);
		int sw = source.width();
		int sh = source.height();
		for (int y = 0; y < height; y++) {
			int sy = y < border ? y : y >= height - border ? sh - (height - y) : border + (y - border) % Math.max(1, sh - 2 * border);
			for (int x = 0; x < width; x++) {
				int sx = x < border ? x : x >= width - border ? sw - (width - x) : border + (x - border) % Math.max(1, sw - 2 * border);
				result.set(x, y, source.get(Math.min(sw - 1, sx), Math.min(sh - 1, sy)));
			}
		}
		return result;
	}
}
