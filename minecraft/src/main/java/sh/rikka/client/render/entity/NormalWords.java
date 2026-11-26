package sh.rikka.client.render.entity;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.util.Mth;
import org.joml.Math;
import org.joml.Matrix3f;
import sh.rikka.client.mixin.entity.PoseAccessor;

final class NormalWords {
	private NormalWords() {
	}

	static int transform(PoseStack.Pose pose, float x, float y, float z) {
		Matrix3f matrix = pose.normal();
		float normalX = Math.fma(matrix.m00(), x, Math.fma(matrix.m10(), y, matrix.m20() * z));
		float normalY = Math.fma(matrix.m01(), x, Math.fma(matrix.m11(), y, matrix.m21() * z));
		float normalZ = Math.fma(matrix.m02(), x, Math.fma(matrix.m12(), y, matrix.m22() * z));
		if (!((PoseAccessor) (Object) pose).rikka$trustedNormals()) {
			float inverseLength = Math.invsqrt(Math.fma(normalX, normalX, Math.fma(normalY, normalY, normalZ * normalZ)));
			normalX *= inverseLength;
			normalY *= inverseLength;
			normalZ *= inverseLength;
		}
		return word(component(normalX), component(normalY), component(normalZ));
	}

	static int negate(int word) {
		return word(-(byte) word, -(byte) (word >> 8), -(byte) (word >> 16));
	}

	private static int component(float value) {
		return (int) (Mth.clamp(value, -1.0F, 1.0F) * 127.0F);
	}

	private static int word(int x, int y, int z) {
		return x & 0xFF | (y & 0xFF) << 8 | (z & 0xFF) << 16;
	}
}
