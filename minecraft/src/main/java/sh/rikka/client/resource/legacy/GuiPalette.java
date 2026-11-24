package sh.rikka.client.resource.legacy;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

final class GuiPalette {
	private static final int MIN_SAMPLES = 8;

	private final Map<Integer, Integer> exact;
	private final TreeMap<Integer, Integer> grays;

	private GuiPalette(Map<Integer, Integer> exact, TreeMap<Integer, Integer> grays) {
		this.exact = exact;
		this.grays = grays;
	}

	public static GuiPalette learn(Raster pack, Raster vanilla) {
		double scale = pack.scale(vanilla.width());
		Map<Integer, List<Integer>> samples = new HashMap<>();
		int height = Math.min(vanilla.height(), (int) (pack.height() / scale));
		for (int y = 0; y < height; y++) {
			for (int x = 0; x < vanilla.width(); x++) {
				int reference = vanilla.get(x, y);
				if (Colors.alpha(reference) == 0) {
					continue;
				}
				int px = Math.min(pack.width() - 1, (int) ((x + 0.5) * scale));
				int py = Math.min(pack.height() - 1, (int) ((y + 0.5) * scale));
				samples.computeIfAbsent(reference, key -> new ArrayList<>()).add(pack.get(px, py));
			}
		}
		Map<Integer, Integer> exact = new HashMap<>();
		TreeMap<Integer, Integer> grays = new TreeMap<>();
		for (Map.Entry<Integer, List<Integer>> entry : samples.entrySet()) {
			if (entry.getValue().size() < MIN_SAMPLES) {
				continue;
			}
			int median = median(entry.getValue());
			exact.put(entry.getKey(), median);
			if (isGray(entry.getKey()) && Colors.alpha(entry.getKey()) == 255) {
				grays.put(Colors.red(entry.getKey()), median);
			}
		}
		return new GuiPalette(exact, grays);
	}

	public int map(int vanilla) {
		Integer mapped = this.exact.get(vanilla);
		if (mapped != null) {
			return mapped;
		}
		if (!isGray(vanilla) || this.grays.isEmpty()) {
			return vanilla;
		}
		int level = Colors.red(vanilla);
		Map.Entry<Integer, Integer> below = this.grays.floorEntry(level);
		Map.Entry<Integer, Integer> above = this.grays.ceilingEntry(level);
		int color;
		if (below == null) {
			color = above.getValue();
		} else if (above == null || above.getKey().equals(below.getKey())) {
			color = below.getValue();
		} else {
			color = Colors.lerp(below.getValue(), above.getValue(), (double) (level - below.getKey()) / (above.getKey() - below.getKey()));
		}
		return Colors.withAlpha(color, Colors.alpha(color) * Colors.alpha(vanilla) / 255);
	}

	public boolean identity() {
		for (Map.Entry<Integer, Integer> entry : this.exact.entrySet()) {
			if (Colors.distance(entry.getKey(), entry.getValue()) > 3 * 4 * 4) {
				return false;
			}
		}
		return true;
	}

	private static boolean isGray(int argb) {
		return Colors.red(argb) == Colors.green(argb) && Colors.green(argb) == Colors.blue(argb);
	}

	private static int median(List<Integer> colors) {
		int[] alpha = new int[colors.size()];
		int[] red = new int[colors.size()];
		int[] green = new int[colors.size()];
		int[] blue = new int[colors.size()];
		for (int i = 0; i < colors.size(); i++) {
			int color = colors.get(i);
			alpha[i] = Colors.alpha(color);
			red[i] = Colors.red(color);
			green[i] = Colors.green(color);
			blue[i] = Colors.blue(color);
		}
		Arrays.sort(alpha);
		Arrays.sort(red);
		Arrays.sort(green);
		Arrays.sort(blue);
		int middle = colors.size() / 2;
		return Colors.argb(alpha[middle], red[middle], green[middle], blue[middle]);
	}
}
