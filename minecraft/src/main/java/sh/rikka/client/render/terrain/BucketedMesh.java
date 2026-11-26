package sh.rikka.client.render.terrain;

import net.minecraft.client.renderer.chunk.ChunkSectionLayer;
import org.jspecify.annotations.Nullable;

public interface BucketedMesh {
	int @Nullable [] rikka$bucketEnds(ChunkSectionLayer layer);
}
