package sh.rikka.client.mods.minimap;

import it.unimi.dsi.fastutil.longs.LongIterator;
import it.unimi.dsi.fastutil.longs.LongOpenHashSet;
import net.minecraft.client.multiplayer.ClientChunkCache;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.core.SectionPos;
import net.minecraft.util.Util;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import org.jspecify.annotations.Nullable;

final class TerrainCache implements AutoCloseable {
	private static final long SCAN_BUDGET_NANOS = 3_000_000L;
	private static final int DIRTY_PRUNE_THRESHOLD = 2048;
	private static final int LAST_CELL = TerrainScanner.CHUNK - 1;

	private final TerrainPalette palette = new TerrainPalette();
	private final TerrainScanner scanner = new TerrainScanner();
	private final LongOpenHashSet dirty = new LongOpenHashSet();
	private @Nullable TerrainAtlas atlas;
	private @Nullable ClientLevel level;
	private int generation;

	TerrainAtlas atlas() {
		TerrainAtlas current = this.atlas;
		if (current == null) {
			current = new TerrainAtlas();
			this.atlas = current;
		}
		return current;
	}

	void markChunk(int chunkX, int chunkZ) {
		this.dirty.add(ChunkPos.pack(chunkX, chunkZ));
		this.dirty.add(ChunkPos.pack(chunkX, chunkZ + 1));
		this.dirty.add(ChunkPos.pack(chunkX + 1, chunkZ));
	}

	void markBlock(BlockPos pos) {
		int chunkX = SectionPos.blockToSectionCoord(pos.getX());
		int chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
		this.dirty.add(ChunkPos.pack(chunkX, chunkZ));
		if ((pos.getZ() & LAST_CELL) == LAST_CELL) {
			this.dirty.add(ChunkPos.pack(chunkX, chunkZ + 1));
		}
		if ((pos.getX() & LAST_CELL) == LAST_CELL) {
			this.dirty.add(ChunkPos.pack(chunkX + 1, chunkZ));
		}
	}

	void reloadColors() {
		this.palette.clear();
		this.generation++;
	}

	void detach() {
		this.level = null;
		this.dirty.clear();
	}

	void refresh(ClientLevel level, int centerX, int centerZ, int radius, int ceiling, TerrainColors colors) {
		TerrainAtlas atlas = atlas();
		if (level != this.level) {
			this.level = level;
			this.dirty.clear();
			atlas.clear();
		}
		if (this.dirty.size() > DIRTY_PRUNE_THRESHOLD) {
			prune(centerX, centerZ, radius);
		}
		long key = (long) this.generation << 40 ^ (long) colors.ordinal() << 32 ^ (ceiling & 0xFFFFFFFFL);
		long deadline = Util.getNanos() + SCAN_BUDGET_NANOS;
		ClientChunkCache chunks = level.getChunkSource();
		for (int ring = 0; ring <= radius; ring++) {
			for (int dz = -ring; dz <= ring; dz++) {
				boolean edgeRow = dz == -ring || dz == ring;
				int step = edgeRow ? 1 : ring * 2;
				for (int dx = -ring; dx <= ring; dx += Math.max(step, 1)) {
					if (!visit(level, chunks, atlas, centerX + dx, centerZ + dz, ceiling, colors, key, deadline)) {
						return;
					}
				}
			}
		}
	}

	private boolean visit(
		ClientLevel level,
		ClientChunkCache chunks,
		TerrainAtlas atlas,
		int chunkX,
		int chunkZ,
		int ceiling,
		TerrainColors colors,
		long key,
		long deadline
	) {
		long pos = ChunkPos.pack(chunkX, chunkZ);
		boolean stale = this.dirty.contains(pos);
		if (!stale && atlas.holds(chunkX, chunkZ, key)) {
			return true;
		}
		LevelChunk chunk = chunks.getChunk(chunkX, chunkZ, ChunkStatus.FULL, false);
		if (chunk == null) {
			this.dirty.remove(pos);
			if (atlas.occupiedBy(chunkX, chunkZ)) {
				return true;
			}
			if (Util.getNanos() > deadline) {
				return false;
			}
			atlas.vacate(chunkX, chunkZ, key);
			return true;
		}
		if (Util.getNanos() > deadline) {
			return false;
		}
		LevelChunk north = chunks.getChunk(chunkX, chunkZ - 1, ChunkStatus.FULL, false);
		LevelChunk west = chunks.getChunk(chunkX - 1, chunkZ, ChunkStatus.FULL, false);
		atlas.write(chunkX, chunkZ, key, this.scanner.scan(level, chunk, north, west, ceiling, colors, this.palette));
		this.dirty.remove(pos);
		return true;
	}

	private void prune(int centerX, int centerZ, int radius) {
		LongIterator iterator = this.dirty.iterator();
		while (iterator.hasNext()) {
			long pos = iterator.nextLong();
			if (Math.abs(ChunkPos.getX(pos) - centerX) > radius || Math.abs(ChunkPos.getZ(pos) - centerZ) > radius) {
				iterator.remove();
			}
		}
	}

	@Override
	public void close() {
		TerrainAtlas current = this.atlas;
		if (current != null) {
			current.close();
			this.atlas = null;
		}
		detach();
		this.palette.clear();
	}
}
