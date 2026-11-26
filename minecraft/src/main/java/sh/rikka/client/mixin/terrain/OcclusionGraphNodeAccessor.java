package sh.rikka.client.mixin.terrain;

import net.minecraft.client.renderer.SectionOcclusionGraph;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(SectionOcclusionGraph.Node.class)
public interface OcclusionGraphNodeAccessor {
	@Accessor("section")
	SectionRenderDispatcher.RenderSection rikka$section();
}
