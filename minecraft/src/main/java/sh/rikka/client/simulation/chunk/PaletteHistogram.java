package sh.rikka.client.simulation.chunk;

import it.unimi.dsi.fastutil.ints.Int2IntOpenHashMap;
import net.minecraft.util.BitStorage;

public final class PaletteHistogram {
	private static final int MAXIMUM_BITS = 8;

	private PaletteHistogram() {
	}

	public static boolean supports(final BitStorage storage) {
		return storage.getBits() <= MAXIMUM_BITS;
	}

	public static void countInto(final BitStorage storage, final Int2IntOpenHashMap counts) {
		int size = storage.getSize();
		int[] ids = PaletteScratch.ofSize(size);
		storage.unpack(ids);
		int range = 1 << storage.getBits();
		int[] histogram = new int[range];
		int[] firstSeen = new int[range];
		int distinct = 0;
		for (int index = 0; index < size; index++) {
			int id = ids[index];
			if (histogram[id]++ == 0) {
				firstSeen[distinct++] = id;
			}
		}
		for (int index = 0; index < distinct; index++) {
			int id = firstSeen[index];
			counts.addTo(id, histogram[id]);
		}
	}
}
