package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.buffers.Std140Builder;
import com.mojang.blaze3d.buffers.Std140SizeCalculator;
import java.nio.ByteBuffer;
import net.minecraft.client.renderer.DynamicGpuDataStorage;

record FrameBlendUniforms(float inverseTotalWeight, float[] weights) implements DynamicGpuDataStorage.DynamicGpuData {
	static final String BLOCK = "FrameBlend";
	private static final int WEIGHTS_PER_VECTOR = 4;
	private static final int WEIGHT_VECTORS = MotionBlurPipelines.MAX_BLENDED_FRAMES / WEIGHTS_PER_VECTOR;
	static final int SIZE = size();

	@Override
	public void write(ByteBuffer buffer) {
		Std140Builder builder = Std140Builder.intoBuffer(buffer).putFloat(this.inverseTotalWeight).putInt(this.weights.length);
		for (int vector = 0; vector < WEIGHT_VECTORS; vector++) {
			int first = vector * WEIGHTS_PER_VECTOR;
			builder.putVec4(weight(first), weight(first + 1), weight(first + 2), weight(first + 3));
		}
	}

	private float weight(int index) {
		return index < this.weights.length ? this.weights[index] : 0.0F;
	}

	private static int size() {
		Std140SizeCalculator calculator = new Std140SizeCalculator().putFloat().putInt();
		for (int vector = 0; vector < WEIGHT_VECTORS; vector++) {
			calculator.putVec4();
		}
		return calculator.get();
	}
}
