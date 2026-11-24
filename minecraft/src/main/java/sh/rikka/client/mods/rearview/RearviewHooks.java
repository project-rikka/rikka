package sh.rikka.client.mods.rearview;

import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.util.profiling.Profiler;
import net.minecraft.util.profiling.ProfilerFiller;
import sh.rikka.client.RikkaClient;

public final class RearviewHooks {
	private static final String PROFILER_SECTION = "rearview";

	private RearviewHooks() {
	}

	public static void levelDrawn(CameraRenderState camera, boolean skyAllowed) {
		RearviewMod mod = RearviewMod.active();
		if (mod == null) {
			return;
		}
		ProfilerFiller profiler = Profiler.get();
		profiler.push(PROFILER_SECTION);
		try {
			mod.render(new RearviewRenderer.MainView(camera, skyAllowed));
		} catch (RuntimeException failure) {
			RikkaClient.LOGGER.error("rearview failed to render and is disabled", failure);
			mod.setEnabled(false);
		} finally {
			profiler.pop();
		}
	}
}
