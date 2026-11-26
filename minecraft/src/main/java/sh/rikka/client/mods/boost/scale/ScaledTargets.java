package sh.rikka.client.mods.boost.scale;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.pipeline.TextureTarget;
import com.mojang.renderpearl.api.GpuFormat;

final class ScaledTargets {
	private final TextureTarget world;
	private final TextureTarget hand;
	private final TextureTarget outline;

	ScaledTargets(int width, int height) {
		this.world = new TextureTarget("Scaled world", width, height, GpuFormat.RGBA8_UNORM, GpuFormat.D32_FLOAT);
		this.hand = new TextureTarget("Scaled 3d hud depth", width, height, null, GpuFormat.D32_FLOAT);
		this.outline = new TextureTarget("Scaled entity outline", width, height, GpuFormat.RGBA8_UNORM, null);
	}

	RenderTarget world() {
		return this.world;
	}

	void fit(int width, int height) {
		if (this.world.width == width && this.world.height == height) {
			return;
		}
		this.world.resize(width, height);
		this.hand.resize(width, height);
		this.outline.resize(width, height);
	}

	void exchange(RenderTarget main, RenderTarget hand, RenderTarget outline) {
		Attachments.exchange(main, this.world);
		Attachments.exchange(hand, this.hand);
		Attachments.exchange(outline, this.outline);
	}

	void close() {
		this.world.destroyBuffers();
		this.hand.destroyBuffers();
		this.outline.destroyBuffers();
	}
}
