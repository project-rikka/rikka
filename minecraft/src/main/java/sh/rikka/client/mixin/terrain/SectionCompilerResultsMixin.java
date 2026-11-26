package sh.rikka.client.mixin.terrain;

import net.minecraft.client.renderer.chunk.ChunkSectionLayer;
import net.minecraft.client.renderer.chunk.SectionCompiler;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import sh.rikka.client.render.terrain.BucketedResults;
import sh.rikka.client.render.terrain.CompactResults;

@Mixin(SectionCompiler.Results.class)
abstract class SectionCompilerResultsMixin implements BucketedResults, CompactResults {
	@Unique
	private final int[][] rikka$bucketEnds = new int[ChunkSectionLayer.values().length][];

	@Unique
	private boolean rikka$compact;

	@Override
	public void rikka$bucketEnds(ChunkSectionLayer layer, int[] ends) {
		this.rikka$bucketEnds[layer.ordinal()] = ends;
	}

	@Override
	public int @Nullable [] rikka$bucketEnds(ChunkSectionLayer layer) {
		return this.rikka$bucketEnds[layer.ordinal()];
	}

	@Override
	public void rikka$compact(boolean compact) {
		this.rikka$compact = compact;
	}

	@Override
	public boolean rikka$compact() {
		return this.rikka$compact;
	}
}
