package sh.rikka.client.mods.rearview;

import com.mojang.renderpearl.api.textures.GpuSampler;
import it.unimi.dsi.fastutil.objects.ObjectArrayList;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.client.renderer.state.level.LevelRenderState;
import org.jspecify.annotations.Nullable;

public interface RearviewLevel {
	void rikka$overrideSections(@Nullable ObjectArrayList<SectionRenderDispatcher.RenderSection> sections);

	void rikka$followSections(
		@Nullable ObjectArrayList<SectionRenderDispatcher.RenderSection> visible,
		@Nullable ObjectArrayList<SectionRenderDispatcher.RenderSection> nearby
	);

	ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$sectionsToUpdate(ObjectArrayList<SectionRenderDispatcher.RenderSection> visible);

	@Nullable GpuSampler rikka$chunkLayerSampler();

	void rikka$submitFeatures(LevelRenderState state, SubmitNodeCollector collector);
}
