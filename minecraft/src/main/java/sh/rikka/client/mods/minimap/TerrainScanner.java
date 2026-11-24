package sh.rikka.client.mods.minimap;

import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.util.ARGB;
import net.minecraft.util.Mth;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.MapColor;
import org.jspecify.annotations.Nullable;

final class TerrainScanner {
	static final int SURFACE = Integer.MIN_VALUE;
	static final int CHUNK = 16;

	private static final int NO_HEIGHT = Integer.MIN_VALUE;
	private static final int WALL_COLOR = 0xFF1A1B21;
	private static final int WALL_SEARCH = 3;
	private static final float SLOPE_SHADE = 0.07F;
	private static final int MAX_SLOPE = 3;
	private static final float WATER_BASE_OPACITY = 0.5F;
	private static final float WATER_OPACITY_PER_BLOCK = 0.1F;
	private static final float WATER_MAX_OPACITY = 0.92F;

	private final int[] pixels = new int[CHUNK * CHUNK];
	private final int[] heights = new int[CHUNK * CHUNK];
	private final boolean[] liquid = new boolean[CHUNK * CHUNK];
	private final int[] northHeights = new int[CHUNK];
	private final int[] westHeights = new int[CHUNK];
	private final BlockPos.MutableBlockPos cursor = new BlockPos.MutableBlockPos();
	private final BlockPos.MutableBlockPos floor = new BlockPos.MutableBlockPos();

	int[] scan(
		ClientLevel level,
		LevelChunk chunk,
		@Nullable LevelChunk north,
		@Nullable LevelChunk west,
		int ceiling,
		TerrainColors colors,
		TerrainPalette palette
	) {
		int baseX = chunk.getPos().getMinBlockX();
		int baseZ = chunk.getPos().getMinBlockZ();
		for (int i = 0; i < CHUNK; i++) {
			this.northHeights[i] = north == null ? NO_HEIGHT : columnHeight(level, north, baseX + i, baseZ - 1, ceiling);
			this.westHeights[i] = west == null ? NO_HEIGHT : columnHeight(level, west, baseX - 1, baseZ + i, ceiling);
		}
		for (int z = 0; z < CHUNK; z++) {
			for (int x = 0; x < CHUNK; x++) {
				column(level, chunk, baseX + x, baseZ + z, z * CHUNK + x, ceiling, colors, palette);
			}
		}
		for (int z = 0; z < CHUNK; z++) {
			for (int x = 0; x < CHUNK; x++) {
				int index = z * CHUNK + x;
				if (this.pixels[index] != 0 && !this.liquid[index]) {
					this.pixels[index] = shade(this.pixels[index], slope(index, x, z));
				}
			}
		}
		return this.pixels;
	}

	private void column(ClientLevel level, LevelChunk chunk, int x, int z, int index, int ceiling, TerrainColors colors, TerrainPalette palette) {
		this.liquid[index] = false;
		int y = floorY(level, chunk, x, z, ceiling);
		this.heights[index] = y;
		if (y == NO_HEIGHT) {
			this.pixels[index] = 0;
			return;
		}
		if (y == ceiling + 1) {
			this.pixels[index] = WALL_COLOR;
			this.heights[index] = ceiling;
			return;
		}
		this.cursor.set(x, y, z);
		BlockState state = chunk.getBlockState(this.cursor);
		FluidState fluid = state.getFluidState();
		if (fluid.isEmpty()) {
			this.pixels[index] = palette.color(colors, level, state, this.cursor);
			return;
		}
		int depth = 0;
		BlockState below;
		this.floor.set(this.cursor);
		do {
			this.floor.move(Direction.DOWN);
			below = chunk.getBlockState(this.floor);
			depth++;
		} while (this.floor.getY() > level.getMinY() && !below.getFluidState().isEmpty());
		BlockState surface = state.isFaceSturdy(level, this.cursor, Direction.UP) ? state : fluid.createLegacyBlock();
		int top = palette.color(colors, level, surface, this.cursor);
		if (surface == state) {
			this.pixels[index] = top;
			return;
		}
		this.liquid[index] = true;
		int bottom = palette.color(colors, level, below, this.floor);
		float opacity = Math.min(WATER_MAX_OPACITY, WATER_BASE_OPACITY + depth * WATER_OPACITY_PER_BLOCK);
		this.pixels[index] = bottom == 0 ? top : ARGB.srgbLerp(opacity, bottom, top);
	}

	private int columnHeight(ClientLevel level, LevelChunk chunk, int x, int z, int ceiling) {
		int y = floorY(level, chunk, x, z, ceiling);
		return y == ceiling + 1 ? ceiling : y;
	}

	private int floorY(ClientLevel level, LevelChunk chunk, int x, int z, int ceiling) {
		int minY = level.getMinY();
		int top = chunk.getHeight(Heightmap.Types.WORLD_SURFACE, x, z);
		if (top < minY) {
			return NO_HEIGHT;
		}
		int y = ceiling == SURFACE ? top : Math.min(ceiling, top);
		this.cursor.set(x, y, z);
		if (ceiling != SURFACE && isOpaque(level, chunk)) {
			int limit = y - WALL_SEARCH;
			while (y > minY && y > limit && isOpaque(level, chunk)) {
				this.cursor.setY(--y);
			}
			if (isOpaque(level, chunk)) {
				return ceiling + 1;
			}
		}
		while (y > minY && !isOpaque(level, chunk)) {
			this.cursor.setY(--y);
		}
		return isOpaque(level, chunk) ? y : NO_HEIGHT;
	}

	private boolean isOpaque(ClientLevel level, LevelChunk chunk) {
		return chunk.getBlockState(this.cursor).getMapColor(level, this.cursor) != MapColor.NONE;
	}

	private int slope(int index, int x, int z) {
		int height = this.heights[index];
		int north = z > 0 ? this.heights[index - CHUNK] : this.northHeights[x];
		int west = x > 0 ? this.heights[index - 1] : this.westHeights[z];
		int difference = (north == NO_HEIGHT ? 0 : height - north) + (west == NO_HEIGHT ? 0 : height - west);
		return Mth.clamp(difference, -MAX_SLOPE, MAX_SLOPE);
	}

	private static int shade(int argb, int slope) {
		return ARGB.scaleRGB(argb, 1.0F + slope * SLOPE_SHADE);
	}
}
