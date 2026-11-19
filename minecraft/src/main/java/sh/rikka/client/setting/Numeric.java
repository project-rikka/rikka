package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import com.mojang.serialization.DataResult;
import java.math.BigDecimal;

final class Numeric {
	static final Codec<Double> FINITE_DOUBLE = Codec.DOUBLE.validate(
		value -> Double.isFinite(value) ? DataResult.success(value) : DataResult.error(() -> "number is not finite: " + value)
	);

	private Numeric() {
	}

	static void requireBounds(int min, int max) {
		if (min > max) {
			throw new IllegalArgumentException("min " + min + " is above max " + max);
		}
	}

	static void requireBounds(double min, double max) {
		requireFinite(min);
		requireFinite(max);
		if (min > max) {
			throw new IllegalArgumentException("min " + min + " is above max " + max);
		}
	}

	static void requireFinite(double value) {
		if (!Double.isFinite(value)) {
			throw new IllegalArgumentException("number is not finite: " + value);
		}
	}

	static void requireStep(int step, int min, int max) {
		if (step <= 0 || step > (long) max - min && min != max) {
			throw new IllegalArgumentException("invalid step " + step + " for " + min + ".." + max);
		}
	}

	static void requireStep(double step, double min, double max) {
		requireFinite(step);
		if (step <= 0 || step > max - min && min != max) {
			throw new IllegalArgumentException("invalid step " + step + " for " + min + ".." + max);
		}
	}

	static int snap(int value, int min, int max, int step) {
		int clamped = Math.clamp(value, min, max);
		long snapped = min + Math.round(((long) clamped - min) / (double) step) * step;
		return (int) (snapped > max ? snapped - step : snapped);
	}

	static double snap(double value, double min, double max, double step) {
		requireFinite(value);
		double clamped = Math.clamp(value, min, max);
		if (step == 0) {
			return clamped;
		}
		long steps = Math.round((clamped - min) / step);
		double snapped = stepValue(min, step, steps);
		return snapped > max ? stepValue(min, step, steps - 1) : snapped;
	}

	private static double stepValue(double min, double step, long steps) {
		return BigDecimal.valueOf(min).add(BigDecimal.valueOf(step).multiply(BigDecimal.valueOf(steps))).doubleValue();
	}
}
