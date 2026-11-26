package sh.rikka.client.render.skin;

import org.joml.Quaternionf;

final class PartRotations {
	private static final int SLOTS = 256;
	private static final int MATRIX_FLOATS = 9;
	private static final int HASH_MULTIPLIER = 0x9E3779B1;
	private static final int HASH_SHIFT = Integer.SIZE - Integer.numberOfTrailingZeros(SLOTS);

	private final Quaternionf quaternion = new Quaternionf();
	private final int[] xBits = new int[SLOTS];
	private final int[] yBits = new int[SLOTS];
	private final int[] zBits = new int[SLOTS];
	private final float[] matrices = new float[SLOTS * MATRIX_FLOATS];

	PartRotations() {
		for (int slot = 0; slot < SLOTS; slot++) {
			store(slot, 0.0F, 0.0F, 0.0F);
		}
	}

	float[] matrices() {
		return this.matrices;
	}

	int find(float xRot, float yRot, float zRot) {
		int x = Float.floatToRawIntBits(xRot);
		int y = Float.floatToRawIntBits(yRot);
		int z = Float.floatToRawIntBits(zRot);
		int slot = ((x * 31 + y) * 31 + z) * HASH_MULTIPLIER >>> HASH_SHIFT;
		if (this.xBits[slot] != x || this.yBits[slot] != y || this.zBits[slot] != z) {
			store(slot, xRot, yRot, zRot);
		}
		return slot * MATRIX_FLOATS;
	}

	private void store(int slot, float xRot, float yRot, float zRot) {
		this.xBits[slot] = Float.floatToRawIntBits(xRot);
		this.yBits[slot] = Float.floatToRawIntBits(yRot);
		this.zBits[slot] = Float.floatToRawIntBits(zRot);
		Quaternionf q = this.quaternion.rotationZYX(zRot, yRot, xRot);
		float w2 = q.w * q.w;
		float x2 = q.x * q.x;
		float y2 = q.y * q.y;
		float z2 = q.z * q.z;
		float zw = q.z * q.w;
		float xy = q.x * q.y;
		float xz = q.x * q.z;
		float yw = q.y * q.w;
		float yz = q.y * q.z;
		float xw = q.x * q.w;
		int matrix = slot * MATRIX_FLOATS;
		this.matrices[matrix] = w2 + x2 - z2 - y2;
		this.matrices[matrix + 1] = xy + xy + zw + zw;
		this.matrices[matrix + 2] = xz + xz - (yw + yw);
		this.matrices[matrix + 3] = xy + xy - (zw + zw);
		this.matrices[matrix + 4] = y2 - z2 + w2 - x2;
		this.matrices[matrix + 5] = yz + yz + xw + xw;
		this.matrices[matrix + 6] = yw + yw + xz + xz;
		this.matrices[matrix + 7] = yz + yz - (xw + xw);
		this.matrices[matrix + 8] = z2 - y2 - x2 + w2;
	}
}
