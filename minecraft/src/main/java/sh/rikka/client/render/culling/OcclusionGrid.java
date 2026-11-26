package sh.rikka.client.render.culling;

import it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap;
import java.util.Arrays;
import net.minecraft.core.SectionPos;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.PalettedContainer;
import org.jspecify.annotations.Nullable;

final class OcclusionGrid {
	private static final int SECTION_SIZE = 16;
	private static final int SECTION_MASK = SECTION_SIZE - 1;
	private static final int WORDS = SECTION_SIZE * SECTION_SIZE * SECTION_SIZE / Long.SIZE;
	private static final long[] FULL = full();
	private static final long NO_SECTION = Long.MIN_VALUE;

	private final Long2ObjectOpenHashMap<long[]> sections = new Long2ObjectOpenHashMap<>();
	private final int minSectionY;
	private final int maxSectionY;
	private long cachedKey = NO_SECTION;
	private long @Nullable [] cachedBits;

	OcclusionGrid(int minSectionY, int maxSectionY) {
		this.minSectionY = minSectionY;
		this.maxSectionY = maxSectionY;
	}

	void apply(GridUpdate update) {
		this.cachedKey = NO_SECTION;
		this.cachedBits = null;
		switch (update) {
			case GridUpdate.ChunkLoaded loaded -> {
				this.unload(loaded.chunkX(), loaded.chunkZ());
				for (GridUpdate.SectionStates section : loaded.sections()) {
					long[] bits = occupancy(section.states());
					if (bits != null) {
						this.sections.put(SectionPos.asLong(loaded.chunkX(), section.sectionY(), loaded.chunkZ()), bits);
					}
				}
			}
			case GridUpdate.ChunkUnloaded unloaded -> this.unload(unloaded.chunkX(), unloaded.chunkZ());
			case GridUpdate.BlockChanged changed -> this.set(changed.x(), changed.y(), changed.z(), changed.opaque());
		}
	}

	boolean isOpaque(int x, int y, int z) {
		long key = SectionPos.asLong(x >> 4, y >> 4, z >> 4);
		if (key != this.cachedKey) {
			this.cachedKey = key;
			this.cachedBits = this.sections.get(key);
		}
		long[] bits = this.cachedBits;
		if (bits == null) {
			return false;
		}
		int index = index(x, y, z);
		return (bits[index >>> 6] & 1L << index) != 0;
	}

	private void unload(int chunkX, int chunkZ) {
		for (int sectionY = this.minSectionY; sectionY <= this.maxSectionY; sectionY++) {
			this.sections.remove(SectionPos.asLong(chunkX, sectionY, chunkZ));
		}
	}

	private void set(int x, int y, int z, boolean opaque) {
		long key = SectionPos.asLong(x >> 4, y >> 4, z >> 4);
		long[] bits = this.sections.get(key);
		if (bits == FULL) {
			if (opaque) {
				return;
			}
			bits = FULL.clone();
			this.sections.put(key, bits);
		} else if (bits == null) {
			if (!opaque) {
				return;
			}
			bits = new long[WORDS];
			this.sections.put(key, bits);
		}
		int index = index(x, y, z);
		if (opaque) {
			bits[index >>> 6] |= 1L << index;
		} else {
			bits[index >>> 6] &= ~(1L << index);
		}
	}

	private static long @Nullable [] occupancy(PalettedContainer<BlockState> states) {
		if (!states.maybeHas(BlockState::isSolidRender)) {
			return null;
		}
		if (!states.maybeHas(state -> !state.isSolidRender())) {
			return FULL;
		}
		long[] bits = new long[WORDS];
		for (int y = 0; y < SECTION_SIZE; y++) {
			for (int z = 0; z < SECTION_SIZE; z++) {
				for (int x = 0; x < SECTION_SIZE; x++) {
					if (states.get(x, y, z).isSolidRender()) {
						int index = index(x, y, z);
						bits[index >>> 6] |= 1L << index;
					}
				}
			}
		}
		return bits;
	}

	private static int index(int x, int y, int z) {
		return (y & SECTION_MASK) << 8 | (z & SECTION_MASK) << 4 | x & SECTION_MASK;
	}

	private static long[] full() {
		long[] bits = new long[WORDS];
		Arrays.fill(bits, -1L);
		return bits;
	}
}
