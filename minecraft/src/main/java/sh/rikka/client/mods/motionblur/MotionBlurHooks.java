package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.pipeline.RenderTarget;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.util.profiling.Profiler;
import net.minecraft.util.profiling.ProfilerFiller;
import org.joml.Matrix4fc;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class MotionBlurHooks {
	private static final Logger LOGGER = LoggerFactory.getLogger(MotionBlurHooks.class);
	private static final String PROFILER_SECTION = "motionBlur";
	private static final MotionBlurRenderer RENDERER = new MotionBlurRenderer();

	private MotionBlurHooks() {
	}

	public static void levelStarting(CameraRenderState camera, Matrix4fc projection) {
		MotionBlurMod mod = MotionBlurMod.active();
		if (mod == null) {
			return;
		}
		Minecraft minecraft = Minecraft.getInstance();
		LocalPlayer player = minecraft.player;
		MotionBlurRenderer.View view = new MotionBlurRenderer.View(
			camera,
			projection,
			minecraft.level,
			minecraft.getWindow(),
			player != null && player.isPassenger()
		);
		run(() -> RENDERER.begin(mod, view));
	}

	static void release() {
		RENDERER.release();
	}

	public static boolean blursTerrain() {
		return RENDERER.blursTerrain();
	}

	public static void terrainDrawn(RenderTarget main) {
		run(() -> RENDERER.terrainDrawn(main));
	}

	public static void levelDrawn(RenderTarget main) {
		run(() -> RENDERER.levelDrawn(main));
	}

	public static void levelFinished(RenderTarget main) {
		run(() -> RENDERER.finish(main));
	}

	public static void displayModeChanged() {
		RENDERER.refreshRateChanged();
	}

	private static void run(Runnable stage) {
		ProfilerFiller profiler = Profiler.get();
		profiler.push(PROFILER_SECTION);
		try {
			stage.run();
		} catch (RuntimeException failure) {
			LOGGER.error("motion blur failed, disabling it", failure);
			RENDERER.release();
			MotionBlurMod mod = MotionBlurMod.active();
			if (mod != null) {
				mod.setEnabled(false);
			}
		} finally {
			profiler.pop();
		}
	}
}
