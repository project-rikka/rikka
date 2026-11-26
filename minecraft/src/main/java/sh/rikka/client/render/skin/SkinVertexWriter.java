package sh.rikka.client.render.skin;

import org.joml.Vector3fc;
import org.lwjgl.system.MemoryUtil;

final class SkinVertexWriter {
	private static final float NORMAL_SCALE = 127.0F;

	private long address;

	SkinVertexWriter(long address) {
		this.address = address;
	}

	void put(float x, float y, float z, float u, float v, Vector3fc normal, int boneRow, int tintRow, int emission) {
		long address = this.address;
		MemoryUtil.memPutFloat(address, x);
		MemoryUtil.memPutFloat(address + 4L, y);
		MemoryUtil.memPutFloat(address + 8L, z);
		MemoryUtil.memPutFloat(address + SkinLayout.VERTEX_UV_OFFSET, u);
		MemoryUtil.memPutFloat(address + SkinLayout.VERTEX_UV_OFFSET + 4L, v);
		MemoryUtil.memPutByte(address + SkinLayout.VERTEX_NORMAL_OFFSET, (byte) (normal.x() * NORMAL_SCALE));
		MemoryUtil.memPutByte(address + SkinLayout.VERTEX_NORMAL_OFFSET + 1L, (byte) (normal.y() * NORMAL_SCALE));
		MemoryUtil.memPutByte(address + SkinLayout.VERTEX_NORMAL_OFFSET + 2L, (byte) (normal.z() * NORMAL_SCALE));
		MemoryUtil.memPutByte(address + SkinLayout.VERTEX_NORMAL_OFFSET + 3L, (byte) 0);
		MemoryUtil.memPutShort(address + SkinLayout.VERTEX_SLOTS_OFFSET, (short) boneRow);
		MemoryUtil.memPutShort(address + SkinLayout.VERTEX_SLOTS_OFFSET + 2L, (short) tintRow);
		MemoryUtil.memPutShort(address + SkinLayout.VERTEX_SLOTS_OFFSET + 4L, (short) emission);
		MemoryUtil.memPutShort(address + SkinLayout.VERTEX_SLOTS_OFFSET + 6L, (short) 0);
		this.address = address + SkinLayout.VERTEX_BYTES;
	}
}
