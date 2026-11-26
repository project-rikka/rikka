package sh.rikka.bench;

import com.mojang.renderpearl.api.commands.RenderPass;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.loader.api.FabricLoader;
import org.jspecify.annotations.Nullable;

public final class BenchMod implements ClientModInitializer {
	private static @Nullable Runner runner;

	@Override
	public void onInitializeClient() {
		Settings settings = Settings.fromEnvironment(FabricLoader.getInstance().getGameDir());
		GameOptions.write(FabricLoader.getInstance().getGameDir(), settings.preset());
		runner = new Runner(settings);
	}

	public static void frame() {
		if (runner != null) {
			runner.frame();
		}
	}

	public static void gpuBegin(GpuSpan span, @Nullable RenderPass pass) {
		if (runner != null) {
			runner.gpuBegin(span, pass);
		}
	}

	public static void gpuEnd(GpuSpan span, @Nullable RenderPass pass) {
		if (runner != null) {
			runner.gpuEnd(span, pass);
		}
	}

	public static void waited(Wait wait, long nanos) {
		if (runner != null) {
			runner.waited(wait, nanos);
		}
	}

	public static void tickStarting() {
		if (runner != null) {
			runner.tickStarting();
		}
	}

	public static void tick() {
		if (runner != null) {
			runner.tick();
		}
	}

	public static boolean animatesTextures() {
		return runner == null || runner.animatesTextures();
	}
}
