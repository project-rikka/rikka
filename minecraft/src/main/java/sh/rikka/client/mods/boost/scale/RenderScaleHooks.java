package sh.rikka.client.mods.boost.scale;

import com.mojang.blaze3d.pipeline.RenderTarget;
import net.minecraft.util.profiling.Profiler;
import net.minecraft.util.profiling.ProfilerFiller;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mixin.scale.LevelRendererAccessor;

public final class RenderScaleHooks {
	private static final String PROFILER_SECTION = "upscale";
	private static final Upscaler UPSCALER = new Upscaler();
	private static final TerrainSampler TERRAIN_SAMPLER = new TerrainSampler();

	private static @Nullable ScaledTargets targets;
	private static @Nullable ScaledTargets exchanged;

	private RenderScaleHooks() {
	}

	public static void levelStarting(RenderTarget main, RenderTarget hand, LevelRendererAccessor level) {
		double factor = RenderScale.factor();
		if (factor >= RenderScale.NATIVE) {
			release();
			return;
		}
		int width = scaled(main.width, factor);
		int height = scaled(main.height, factor);
		ScaledTargets scaled = targets;
		if (scaled == null) {
			scaled = new ScaledTargets(width, height);
			targets = scaled;
		} else {
			scaled.fit(width, height);
		}
		TERRAIN_SAMPLER.install(level, mipBias(width, main.width));
		scaled.exchange(main, hand, level.rikka$entityOutlineTarget());
		exchanged = scaled;
	}

	public static void levelFinished(RenderTarget main, RenderTarget hand, LevelRendererAccessor level) {
		ScaledTargets scaled = exchanged;
		if (scaled == null) {
			return;
		}
		exchanged = null;
		TERRAIN_SAMPLER.restore(level);
		scaled.exchange(main, hand, level.rikka$entityOutlineTarget());
		ProfilerFiller profiler = Profiler.get();
		profiler.push(PROFILER_SECTION);
		UPSCALER.apply(scaled.world(), main, RenderScale.sharpness());
		profiler.pop();
	}

	private static void release() {
		ScaledTargets scaled = targets;
		if (scaled == null) {
			return;
		}
		targets = null;
		scaled.close();
		UPSCALER.close();
		TERRAIN_SAMPLER.close();
	}

	private static int scaled(int extent, double factor) {
		return Math.max(1, (int) Math.round(extent * factor));
	}

	private static float mipBias(int scaledWidth, int nativeWidth) {
		return (float) (Math.log((double) scaledWidth / nativeWidth) / Math.log(2.0));
	}
}
