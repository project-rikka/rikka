package sh.rikka.client.simulation.chunk;

final class PaletteScratch {
	private static final int BLOCK_ENTRIES = 4096;
	private static final int BIOME_ENTRIES = 64;
	private static final ThreadLocal<int[]> BLOCKS = ThreadLocal.withInitial(() -> new int[BLOCK_ENTRIES]);
	private static final ThreadLocal<int[]> BIOMES = ThreadLocal.withInitial(() -> new int[BIOME_ENTRIES]);

	private PaletteScratch() {
	}

	static int[] ofSize(final int size) {
		return switch (size) {
			case BLOCK_ENTRIES -> BLOCKS.get();
			case BIOME_ENTRIES -> BIOMES.get();
			default -> new int[size];
		};
	}
}
