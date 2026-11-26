package sh.rikka.client.mods.boost.scale;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.renderpearl.api.textures.GpuTexture;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mixin.scale.RenderTargetAccessor;

record Attachments(
	int width,
	int height,
	@Nullable GpuTexture color,
	@Nullable GpuTextureView colorView,
	@Nullable GpuTexture depth,
	@Nullable GpuTextureView depthView
) {
	static Attachments of(RenderTarget target) {
		return new Attachments(
			target.width,
			target.height,
			target.getColorTexture(),
			target.getColorTextureView(),
			target.getDepthTexture(),
			target.getDepthTextureView()
		);
	}

	static void exchange(RenderTarget first, RenderTarget second) {
		Attachments held = of(first);
		of(second).moveInto(first);
		held.moveInto(second);
	}

	private void moveInto(RenderTarget target) {
		RenderTargetAccessor accessor = (RenderTargetAccessor) target;
		target.width = this.width;
		target.height = this.height;
		accessor.rikka$setColorTexture(this.color);
		accessor.rikka$setColorTextureView(this.colorView);
		accessor.rikka$setDepthTexture(this.depth);
		accessor.rikka$setDepthTextureView(this.depthView);
	}
}
