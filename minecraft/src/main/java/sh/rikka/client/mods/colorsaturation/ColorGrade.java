package sh.rikka.client.mods.colorsaturation;

import com.mojang.blaze3d.buffers.Std140Builder;
import com.mojang.blaze3d.buffers.Std140SizeCalculator;
import java.nio.ByteBuffer;
import net.minecraft.client.renderer.DynamicGpuDataStorage;
import org.joml.Vector4f;

record ColorGrade(boolean neutral, Vector4f red, Vector4f green, Vector4f blue) implements DynamicGpuDataStorage.DynamicGpuData {
	static final String BLOCK = "ColorGrade";
	static final int SIZE = new Std140SizeCalculator().putVec4().putVec4().putVec4().align(16).get();

	private static final int CHANNELS = 3;
	private static final double CONTRAST_PIVOT = 0.5;
	private static final double[] LUMA = {0.213, 0.715, 0.072};
	private static final double[][] HUE_SKEW = {
		{-0.213, -0.715, 0.928},
		{0.143, 0.140, -0.283},
		{-0.787, 0.715, 0.072},
	};

	static ColorGrade of(double hueDegrees, double saturation, double contrast, double brightness) {
		boolean neutral = hueDegrees == 0.0 && saturation == 1.0 && contrast == 1.0 && brightness == 0.0;
		double angle = Math.toRadians(hueDegrees);
		double cos = Math.cos(angle);
		double sin = Math.sin(angle);
		float offset = (float) (CONTRAST_PIVOT * (1.0 - contrast) + brightness);
		Vector4f[] rows = new Vector4f[CHANNELS];
		for (int row = 0; row < CHANNELS; row++) {
			double[] coefficients = new double[CHANNELS];
			for (int column = 0; column < CHANNELS; column++) {
				for (int k = 0; k < CHANNELS; k++) {
					coefficients[column] += saturation(saturation, row, k) * hueRotation(cos, sin, k, column);
				}
			}
			rows[row] = new Vector4f(
				(float) (contrast * coefficients[0]),
				(float) (contrast * coefficients[1]),
				(float) (contrast * coefficients[2]),
				offset
			);
		}
		return new ColorGrade(neutral, rows[0], rows[1], rows[2]);
	}

	@Override
	public void write(ByteBuffer buffer) {
		Std140Builder.intoBuffer(buffer).putVec4(this.red).putVec4(this.green).putVec4(this.blue);
	}

	private static double saturation(double saturation, int row, int column) {
		return (1.0 - saturation) * LUMA[column] + (row == column ? saturation : 0.0);
	}

	private static double hueRotation(double cos, double sin, int row, int column) {
		return LUMA[column] + cos * ((row == column ? 1.0 : 0.0) - LUMA[column]) + sin * HUE_SKEW[row][column];
	}
}
