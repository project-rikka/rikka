package sh.rikka.client.mixin.terrain;

import com.mojang.blaze3d.vertex.MeshData;
import java.util.Map;
import net.minecraft.client.renderer.chunk.ChunkSectionLayer;
import net.minecraft.client.renderer.chunk.SectionCompiler;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import sh.rikka.client.mods.boost.BoostFeature;
import sh.rikka.client.render.terrain.BucketedResults;
import sh.rikka.client.render.terrain.CompactResults;
import sh.rikka.client.render.terrain.CompactVertices;
import sh.rikka.client.render.terrain.QuadBuckets;

@Mixin(SectionCompiler.class)
abstract class SectionCompilerMixin {
	@Inject(method = "compile", at = @At("RETURN"))
	private void rikka$prepareMeshes(CallbackInfoReturnable<SectionCompiler.Results> callback) {
		SectionCompiler.Results results = callback.getReturnValue();
		boolean compact = BoostFeature.COMPACT_VERTICES.isActive();
		for (Map.Entry<ChunkSectionLayer, MeshData> rendered : results.renderedLayers.entrySet()) {
			ChunkSectionLayer layer = rendered.getKey();
			MeshData mesh = rendered.getValue();
			MeshData.DrawState state = mesh.drawState();
			if (!layer.translucent()) {
				int[] ends = QuadBuckets.sort(mesh.vertexBuffer(), state.format(), state.vertexCount());
				((BucketedResults) (Object) results).rikka$bucketEnds(layer, ends);
			}
			if (compact) {
				int bytes = CompactVertices.compact(mesh.vertexBuffer(), state.format(), state.vertexCount());
				((ByteBufferBuilderResultAccessor) mesh.vertexBufferSlice()).rikka$setSize(bytes);
			}
		}
		((CompactResults) (Object) results).rikka$compact(compact);
	}
}
