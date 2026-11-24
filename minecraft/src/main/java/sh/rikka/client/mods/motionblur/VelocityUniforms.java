package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.buffers.Std140Builder;
import com.mojang.blaze3d.buffers.Std140SizeCalculator;
import java.nio.ByteBuffer;
import net.minecraft.client.renderer.DynamicGpuDataStorage;

record VelocityUniforms(CameraStep camera, VelocityStrength strength, BlurProfile profile) implements DynamicGpuDataStorage.DynamicGpuData {
	static final String BLOCK = "VelocityBlur";
	static final int SIZE = new Std140SizeCalculator()
		.putMat4f()
		.putMat4f()
		.putMat4f()
		.putMat4f()
		.putVec3()
		.putFloat()
		.putInt()
		.putInt()
		.align(16)
		.get();

	@Override
	public void write(ByteBuffer buffer) {
		Std140Builder.intoBuffer(buffer)
			.putMat4f(this.camera.viewInverse())
			.putMat4f(this.camera.projectionInverse())
			.putMat4f(this.camera.previousView())
			.putMat4f(this.camera.previousProjection())
			.putVec3(this.camera.delta())
			.putFloat(this.strength.strength())
			.putInt(this.strength.samples())
			.putInt(this.profile == BlurProfile.WEIGHTED ? 1 : 0);
	}
}
