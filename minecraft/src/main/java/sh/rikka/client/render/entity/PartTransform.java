package sh.rikka.client.render.entity;

import com.mojang.blaze3d.vertex.PoseStack;
import org.joml.Math;
import org.joml.Matrix4f;
import org.joml.Matrix4fc;

final class PartTransform {
	private final int[] normalWords = new int[CubeMesh.NORMAL_DIRECTIONS];
	private int properties;
	private float m00;
	private float m01;
	private float m02;
	private float m10;
	private float m11;
	private float m12;
	private float m20;
	private float m21;
	private float m22;
	private float m30;
	private float m31;
	private float m32;

	void load(PoseStack.Pose pose) {
		Matrix4f matrix = pose.pose();
		this.properties = matrix.properties();
		this.m00 = matrix.m00();
		this.m01 = matrix.m01();
		this.m02 = matrix.m02();
		this.m10 = matrix.m10();
		this.m11 = matrix.m11();
		this.m12 = matrix.m12();
		this.m20 = matrix.m20();
		this.m21 = matrix.m21();
		this.m22 = matrix.m22();
		this.m30 = matrix.m30();
		this.m31 = matrix.m31();
		this.m32 = matrix.m32();
		for (int axis = 0; axis < 3; axis++) {
			int word = NormalWords.transform(pose, axis == 0 ? 1.0F : 0.0F, axis == 1 ? 1.0F : 0.0F, axis == 2 ? 1.0F : 0.0F);
			this.normalWords[CubeMesh.normalDirection(axis, false)] = word;
			this.normalWords[CubeMesh.normalDirection(axis, true)] = NormalWords.negate(word);
		}
	}

	void transformCorners(float[] corners, int cornerCount, long[] cornerXY, long[] cornerZColor, int packedColor) {
		if ((this.properties & Matrix4fc.PROPERTY_IDENTITY) != 0) {
			for (int corner = 0; corner < cornerCount; corner++) {
				int offset = corner * 3;
				cornerXY[corner] = VertexWords.pair(corners[offset], corners[offset + 1]);
				cornerZColor[corner] = VertexWords.pair(corners[offset + 2], packedColor);
			}
		} else if ((this.properties & Matrix4fc.PROPERTY_TRANSLATION) != 0) {
			for (int corner = 0; corner < cornerCount; corner++) {
				int offset = corner * 3;
				cornerXY[corner] = VertexWords.pair(corners[offset] + this.m30, corners[offset + 1] + this.m31);
				cornerZColor[corner] = VertexWords.pair(corners[offset + 2] + this.m32, packedColor);
			}
		} else {
			float m00 = this.m00;
			float m01 = this.m01;
			float m02 = this.m02;
			float m10 = this.m10;
			float m11 = this.m11;
			float m12 = this.m12;
			float m20 = this.m20;
			float m21 = this.m21;
			float m22 = this.m22;
			float m30 = this.m30;
			float m31 = this.m31;
			float m32 = this.m32;
			for (int corner = 0; corner < cornerCount; corner++) {
				int offset = corner * 3;
				float x = corners[offset];
				float y = corners[offset + 1];
				float z = corners[offset + 2];
				float transformedX = Math.fma(m00, x, Math.fma(m10, y, Math.fma(m20, z, m30)));
				float transformedY = Math.fma(m01, x, Math.fma(m11, y, Math.fma(m21, z, m31)));
				float transformedZ = Math.fma(m02, x, Math.fma(m12, y, Math.fma(m22, z, m32)));
				cornerXY[corner] = VertexWords.pair(transformedX, transformedY);
				cornerZColor[corner] = VertexWords.pair(transformedZ, packedColor);
			}
		}
	}

	int normalWord(int direction) {
		return this.normalWords[direction];
	}
}
