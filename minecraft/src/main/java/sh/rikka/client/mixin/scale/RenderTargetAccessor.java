package sh.rikka.client.mixin.scale;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.renderpearl.api.textures.GpuTexture;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(RenderTarget.class)
public interface RenderTargetAccessor {
	@Accessor("colorTexture")
	void rikka$setColorTexture(@Nullable GpuTexture texture);

	@Accessor("colorTextureView")
	void rikka$setColorTextureView(@Nullable GpuTextureView view);

	@Accessor("depthTexture")
	void rikka$setDepthTexture(@Nullable GpuTexture texture);

	@Accessor("depthTextureView")
	void rikka$setDepthTextureView(@Nullable GpuTextureView view);
}
