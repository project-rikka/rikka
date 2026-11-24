package sh.rikka.client.mods.colorsaturation;

import com.mojang.blaze3d.pipeline.RenderTarget;
import net.minecraft.util.profiling.Profiler;
import net.minecraft.util.profiling.ProfilerFiller;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class ColorSaturationHooks {
	private static final Logger LOGGER = LoggerFactory.getLogger(ColorSaturationHooks.class);
	private static final String PROFILER_SECTION = "colorGrade";
	private static final ColorGradePass PASS = new ColorGradePass();

	private ColorSaturationHooks() {
	}

	static void release() {
		PASS.close();
	}

	public static void levelFinished(RenderTarget main) {
		ColorSaturationMod mod = ColorSaturationMod.active();
		if (mod == null) {
			return;
		}
		ColorGrade grade = mod.grade();
		if (grade.neutral()) {
			PASS.close();
			return;
		}
		ProfilerFiller profiler = Profiler.get();
		profiler.push(PROFILER_SECTION);
		try {
			PASS.apply(grade, main);
		} catch (RuntimeException failure) {
			LOGGER.error("color saturation failed, disabling it", failure);
			mod.setEnabled(false);
		} finally {
			profiler.pop();
		}
	}
}
