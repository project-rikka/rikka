package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.buffers.Std140Builder;
import com.mojang.blaze3d.buffers.Std140SizeCalculator;
import java.nio.ByteBuffer;
import net.minecraft.client.renderer.DynamicGpuDataStorage;

record AccumulationUniforms(float blendFactor) implements DynamicGpuDataStorage.DynamicGpuData {
	static final String BLOCK = "Accumulation";
	static final int SIZE = new Std140SizeCalculator().putFloat().align(16).get();

	@Override
	public void write(ByteBuffer buffer) {
		Std140Builder.intoBuffer(buffer).putFloat(this.blendFactor);
	}
}
