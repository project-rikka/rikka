package sh.rikka.client.simulation.light;

import it.unimi.dsi.fastutil.longs.Long2ObjectMap;
import it.unimi.dsi.fastutil.longs.Long2ObjectMaps;
import it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap;
import it.unimi.dsi.fastutil.longs.LongIterator;
import it.unimi.dsi.fastutil.longs.LongSet;
import net.minecraft.world.level.chunk.DataLayer;
import org.jspecify.annotations.Nullable;

public final class LightSectionSnapshot {
	private static final int DIRECTORY_BITS = 6;
	private static final int DIRECTORY_SIZE = 1 << DIRECTORY_BITS;
	private static final int DIRECTORY_MASK = DIRECTORY_SIZE - 1;
	private static final int MINIMUM_PAGES = DIRECTORY_SIZE;
	private static final int BUILD_LOAD = 4;
	private static final int GROW_LOAD = 16;
	private static final int SHRINK_LOAD = 1;
	private static final int INITIAL_PAGE_CAPACITY = 8;

	static final LightSectionSnapshot EMPTY = new LightSectionSnapshot(new Object[MINIMUM_PAGES >>> DIRECTORY_BITS][]);

	private final @Nullable Object[][] directories;

	private LightSectionSnapshot(final @Nullable Object[][] directories) {
		this.directories = directories;
	}

	static LightSectionSnapshot of(final Long2ObjectOpenHashMap<DataLayer> source) {
		int pages = Math.max(MINIMUM_PAGES, Integer.highestOneBit(Math.max(1, source.size() / BUILD_LOAD)) * 2);
		LightSectionSnapshot snapshot = new LightSectionSnapshot(new Object[pages >>> DIRECTORY_BITS][]);
		for (Long2ObjectMap.Entry<DataLayer> entry : Long2ObjectMaps.fastIterable(source)) {
			DataLayer layer = entry.getValue();
			if (layer != null) {
				long key = entry.getLongKey();
				long hash = hash(key);
				int page = (int) hash & (pages - 1);
				Object[] directory = snapshot.directories[page >>> DIRECTORY_BITS];
				if (directory == null) {
					directory = new Object[DIRECTORY_SIZE];
					snapshot.directories[page >>> DIRECTORY_BITS] = directory;
				}
				if (directory[page & DIRECTORY_MASK] == null) {
					directory[page & DIRECTORY_MASK] = newPage();
				}
				put(directory, page & DIRECTORY_MASK, key, hash, layer);
			}
		}
		return snapshot;
	}

	public @Nullable DataLayer get(final long key) {
		long hash = hash(key);
		@Nullable Object[][] directories = this.directories;
		int page = (int) hash & ((directories.length << DIRECTORY_BITS) - 1);
		Object[] directory = directories[page >>> DIRECTORY_BITS];
		if (directory == null) {
			return null;
		}
		Object[] entries = (Object[]) directory[page & DIRECTORY_MASK];
		if (entries == null) {
			return null;
		}
		long[] keys = (long[]) entries[0];
		int mask = entries.length - 2;
		int slot = slot(hash) & mask;
		Object layer;
		while ((layer = entries[slot + 1]) != null) {
			if (keys[slot] == key) {
				return (DataLayer) layer;
			}
			slot = (slot + 1) & mask;
		}
		return null;
	}

	int pageCount() {
		return this.directories.length << DIRECTORY_BITS;
	}

	int directoryCount() {
		return this.directories.length;
	}

	boolean fits(final int size) {
		int pages = this.pageCount();
		return size <= pages * GROW_LOAD && (pages == MINIMUM_PAGES || size >= pages * SHRINK_LOAD);
	}

	LightSectionSnapshot with(
		final LongSet changedKeys,
		final Long2ObjectOpenHashMap<DataLayer> source,
		final long[] directoryStamps,
		final long[] pageStamps,
		final long generation
	) {
		@Nullable Object[][] directories = this.directories.clone();
		int pageMask = (directories.length << DIRECTORY_BITS) - 1;
		LongIterator iterator = changedKeys.iterator();
		while (iterator.hasNext()) {
			long key = iterator.nextLong();
			long hash = hash(key);
			int page = (int) hash & pageMask;
			int directoryIndex = page >>> DIRECTORY_BITS;
			int pageIndex = page & DIRECTORY_MASK;
			DataLayer layer = source.get(key);
			Object[] directory = directories[directoryIndex];
			if (directory == null) {
				if (layer == null) {
					continue;
				}
				directory = new Object[DIRECTORY_SIZE];
				directories[directoryIndex] = directory;
				directoryStamps[directoryIndex] = generation;
			} else if (directoryStamps[directoryIndex] != generation) {
				directory = directory.clone();
				directories[directoryIndex] = directory;
				directoryStamps[directoryIndex] = generation;
			}
			Object[] entries = (Object[]) directory[pageIndex];
			if (entries == null) {
				if (layer == null) {
					continue;
				}
				directory[pageIndex] = newPage();
				pageStamps[page] = generation;
			} else if (pageStamps[page] != generation) {
				Object[] copy = entries.clone();
				copy[0] = ((long[]) entries[0]).clone();
				directory[pageIndex] = copy;
				pageStamps[page] = generation;
			}
			if (layer == null) {
				remove(directory, pageIndex, key, hash);
			} else {
				put(directory, pageIndex, key, hash, layer);
			}
		}
		return new LightSectionSnapshot(directories);
	}

	private static Object[] newPage() {
		Object[] entries = new Object[INITIAL_PAGE_CAPACITY + 1];
		entries[0] = new long[INITIAL_PAGE_CAPACITY + 1];
		return entries;
	}

	private static void put(final Object[] directory, final int pageIndex, final long key, final long hash, final DataLayer layer) {
		Object[] entries = (Object[]) directory[pageIndex];
		long[] keys = (long[]) entries[0];
		int capacity = entries.length - 1;
		int mask = capacity - 1;
		int slot = slot(hash) & mask;
		while (entries[slot + 1] != null) {
			if (keys[slot] == key) {
				entries[slot + 1] = layer;
				return;
			}
			slot = (slot + 1) & mask;
		}
		keys[slot] = key;
		entries[slot + 1] = layer;
		long size = ++keys[capacity];
		if (size * 2 > capacity) {
			directory[pageIndex] = grow(entries);
		}
	}

	private static void remove(final Object[] directory, final int pageIndex, final long key, final long hash) {
		Object[] entries = (Object[]) directory[pageIndex];
		long[] keys = (long[]) entries[0];
		int capacity = entries.length - 1;
		int mask = capacity - 1;
		int slot = slot(hash) & mask;
		while (entries[slot + 1] != null) {
			if (keys[slot] == key) {
				keys[capacity]--;
				closeGap(entries, keys, slot, mask);
				return;
			}
			slot = (slot + 1) & mask;
		}
	}

	private static void closeGap(final Object[] entries, final long[] keys, final int start, final int mask) {
		int gap = start;
		int next = start;
		while (true) {
			next = (next + 1) & mask;
			Object layer = entries[next + 1];
			if (layer == null) {
				keys[gap] = 0L;
				entries[gap + 1] = null;
				return;
			}
			int home = slot(hash(keys[next])) & mask;
			if (((next - home) & mask) >= ((next - gap) & mask)) {
				keys[gap] = keys[next];
				entries[gap + 1] = layer;
				gap = next;
			}
		}
	}

	private static Object[] grow(final Object[] oldEntries) {
		long[] oldKeys = (long[]) oldEntries[0];
		int oldCapacity = oldEntries.length - 1;
		int capacity = oldCapacity * 2;
		int mask = capacity - 1;
		Object[] entries = new Object[capacity + 1];
		long[] keys = new long[capacity + 1];
		entries[0] = keys;
		for (int index = 0; index < oldCapacity; index++) {
			Object layer = oldEntries[index + 1];
			if (layer != null) {
				long key = oldKeys[index];
				int slot = slot(hash(key)) & mask;
				while (entries[slot + 1] != null) {
					slot = (slot + 1) & mask;
				}
				keys[slot] = key;
				entries[slot + 1] = layer;
			}
		}
		keys[capacity] = oldKeys[oldCapacity];
		return entries;
	}

	private static long hash(final long key) {
		long hash = key * 0x9e3779b97f4a7c15L;
		hash ^= hash >>> 32;
		return hash ^ (hash >>> 16);
	}

	private static int slot(final long hash) {
		return (int) (hash >>> 40);
	}
}
