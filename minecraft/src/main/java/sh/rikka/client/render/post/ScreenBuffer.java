package sh.rikka.client.render.post;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.pipeline.TextureTarget;
import com.mojang.renderpearl.api.GpuFormat;
import org.jspecify.annotations.Nullable;

public final class ScreenBuffer {
	private final String label;
	private @Nullable TextureTarget target;

	public ScreenBuffer(String label) {
		this.label = label;
	}

	public static TextureTarget allocate(String label, RenderTarget like) {
		return new TextureTarget(label, like.width, like.height, GpuFormat.RGBA8_UNORM, null);
	}

	public static boolean matches(RenderTarget target, RenderTarget like) {
		return target.width == like.width && target.height == like.height;
	}

	public RenderTarget sizedLike(RenderTarget like) {
		TextureTarget target = this.target;
		if (target != null && matches(target, like)) {
			return target;
		}
		close();
		target = allocate(this.label, like);
		this.target = target;
		return target;
	}

	public boolean fits(RenderTarget like) {
		return this.target != null && matches(this.target, like);
	}

	public void close() {
		if (this.target != null) {
			this.target.destroyBuffers();
			this.target = null;
		}
	}
}
