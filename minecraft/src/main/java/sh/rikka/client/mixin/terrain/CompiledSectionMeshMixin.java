package sh.rikka.client.mixin.terrain;

import net.minecraft.client.renderer.chunk.ChunkSectionLayer;
import net.minecraft.client.renderer.chunk.CompiledSectionMesh;
import net.minecraft.client.renderer.chunk.SectionCompiler;
import net.minecraft.client.renderer.chunk.TranslucencyPointOfView;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.render.terrain.BucketedMesh;
import sh.rikka.client.render.terrain.CompactMesh;

@Mixin(CompiledSectionMesh.class)
abstract class CompiledSectionMeshMixin implements BucketedMesh, CompactMesh {
	@Unique
	private final int[][] rikka$bucketEnds = new int[ChunkSectionLayer.values().length][];

	@Unique
	private boolean rikka$compact;

	@Inject(method = "<init>", at = @At("TAIL"))
	private void rikka$keepCompileResults(TranslucencyPointOfView pointOfView, SectionCompiler.Results results, long compileTaskStartTimeNs, CallbackInfo callback) {
		BucketedMesh buckets = (BucketedMesh) (Object) results;
		for (ChunkSectionLayer layer : ChunkSectionLayer.values()) {
			this.rikka$bucketEnds[layer.ordinal()] = buckets.rikka$bucketEnds(layer);
		}
		this.rikka$compact = ((CompactMesh) (Object) results).rikka$compact();
	}

	@Override
	public int @Nullable [] rikka$bucketEnds(ChunkSectionLayer layer) {
		return this.rikka$bucketEnds[layer.ordinal()];
	}

	@Override
	public boolean rikka$compact() {
		return this.rikka$compact;
	}
}
