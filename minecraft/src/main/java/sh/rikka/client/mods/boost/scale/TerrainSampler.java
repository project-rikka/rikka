package sh.rikka.client.mods.boost.scale;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.textures.GpuSampler;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mixin.scale.LevelRendererAccessor;

public final class TerrainSampler {
	private static float creatingBias;

	private @Nullable GpuSampler source;
	private @Nullable GpuSampler biased;
	private float bias;
	private @Nullable GpuSampler held;

	public static float creatingBias() {
		return creatingBias;
	}

	void install(LevelRendererAccessor level, float bias) {
		GpuSampler current = level.rikka$terrainSampler();
		if (current == null) {
			return;
		}
		GpuSampler copy = this.biased;
		if (copy == null || this.source != current || this.bias != bias) {
			close();
			copy = create(current, bias);
			this.biased = copy;
			this.source = current;
			this.bias = bias;
		}
		this.held = current;
		level.rikka$setTerrainSampler(copy);
	}

	void restore(LevelRendererAccessor level) {
		GpuSampler original = this.held;
		if (original == null) {
			return;
		}
		this.held = null;
		if (level.rikka$terrainSampler() == this.biased) {
			level.rikka$setTerrainSampler(original);
			return;
		}
		original.close();
		this.biased = null;
		this.source = null;
	}

	void close() {
		if (this.biased != null) {
			this.biased.close();
			this.biased = null;
			this.source = null;
		}
	}

	private static GpuSampler create(GpuSampler like, float bias) {
		creatingBias = bias;
		try {
			return RenderSystem.getDevice()
				.createSampler(
					like.getAddressModeU(),
					like.getAddressModeV(),
					like.getMinFilter(),
					like.getMagFilter(),
					like.getMaxAnisotropy(),
					like.getMaxLod()
				);
		} finally {
			creatingBias = 0.0F;
		}
	}
}
