package sh.rikka.client.render.terrain;

import net.minecraft.client.renderer.chunk.ChunkSectionLayer;

public interface BucketedResults extends BucketedMesh {
	void rikka$bucketEnds(ChunkSectionLayer layer, int[] ends);
}
