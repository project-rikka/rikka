package sh.rikka.client.render.skin;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import com.mojang.renderpearl.api.commands.CommandEncoder;
import org.jspecify.annotations.Nullable;
import org.lwjgl.system.MemoryUtil;

final class SkinMeshPool implements AutoCloseable {
	private static final long INITIAL_BYTES = 1L << 20;

	private long mirror = MemoryUtil.nmemAlloc(INITIAL_BYTES);
	private long capacity = INITIAL_BYTES;
	private long used;
	private long uploaded;
	private @Nullable GpuBuffer buffer;

	int allocate(int vertexCount) {
		long bytes = (long) vertexCount * SkinLayout.VERTEX_BYTES;
		if (this.used + bytes > this.capacity) {
			this.capacity = Long.highestOneBit(this.used + bytes - 1) << 1;
			this.mirror = MemoryUtil.nmemRealloc(this.mirror, this.capacity);
		}
		int baseVertex = Math.toIntExact(this.used / SkinLayout.VERTEX_BYTES);
		this.used += bytes;
		return baseVertex;
	}

	long address(int vertex) {
		return this.mirror + (long) vertex * SkinLayout.VERTEX_BYTES;
	}

	void upload() {
		if (this.uploaded == this.used) {
			return;
		}
		GpuBuffer buffer = this.buffer;
		if (buffer == null || buffer.size() < this.used) {
			if (buffer != null) {
				buffer.close();
			}
			buffer = RenderSystem.getDevice().createBuffer(() -> "Rikka Skin Meshes", GpuBuffer.USAGE_VERTEX | GpuBuffer.USAGE_COPY_DST, this.capacity);
			this.buffer = buffer;
			this.uploaded = 0L;
		}
		CommandEncoder encoder = RenderSystem.getDevice().createCommandEncoder();
		long length = this.used - this.uploaded;
		encoder.writeToBuffer(buffer.slice(this.uploaded, length), MemoryUtil.memByteBuffer(this.mirror + this.uploaded, Math.toIntExact(length)));
		this.uploaded = this.used;
	}

	GpuBufferSlice vertices() {
		GpuBuffer buffer = this.buffer;
		if (buffer == null) {
			throw new IllegalStateException("skin meshes are not uploaded");
		}
		return buffer.slice();
	}

	void clear() {
		this.used = 0L;
		this.uploaded = 0L;
	}

	@Override
	public void close() {
		if (this.buffer != null) {
			this.buffer.close();
			this.buffer = null;
		}
		MemoryUtil.nmemFree(this.mirror);
		this.mirror = MemoryUtil.NULL;
	}
}
