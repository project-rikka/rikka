package sh.rikka.client.mixin.terrain;

import java.util.concurrent.BlockingQueue;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(targets = "net.minecraft.client.renderer.SectionOcclusionGraph$GraphState")
public interface OcclusionGraphStateAccessor {
	@Accessor("sectionsToPropagateFrom")
	BlockingQueue<SectionRenderDispatcher.RenderSection> rikka$sectionsToPropagateFrom();
}
