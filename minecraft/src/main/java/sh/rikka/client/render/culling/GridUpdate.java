package sh.rikka.client.render.culling;

import java.util.List;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.PalettedContainer;

sealed interface GridUpdate {
	record SectionStates(int sectionY, PalettedContainer<BlockState> states) {
	}

	record ChunkLoaded(int chunkX, int chunkZ, List<SectionStates> sections) implements GridUpdate {
	}

	record ChunkUnloaded(int chunkX, int chunkZ) implements GridUpdate {
	}

	record BlockChanged(int x, int y, int z, boolean opaque) implements GridUpdate {
	}
}
