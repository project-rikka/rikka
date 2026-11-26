package sh.rikka.client.mixin.scale;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.renderpearl.api.textures.GpuSampler;
import net.minecraft.client.renderer.LevelRenderer;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(LevelRenderer.class)
public interface LevelRendererAccessor {
	@Accessor("entityOutlineTarget")
	RenderTarget rikka$entityOutlineTarget();

	@Accessor("chunkLayerSampler")
	@Nullable GpuSampler rikka$terrainSampler();

	@Accessor("chunkLayerSampler")
	void rikka$setTerrainSampler(@Nullable GpuSampler sampler);
}
