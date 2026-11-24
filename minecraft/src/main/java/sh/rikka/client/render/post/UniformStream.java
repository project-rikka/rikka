package sh.rikka.client.render.post;

import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import net.minecraft.client.renderer.DynamicGpuDataStorage;
import net.minecraft.client.renderer.DynamicGpuDataStorageMapped;
import org.jspecify.annotations.Nullable;

public final class UniformStream<T extends DynamicGpuDataStorage.DynamicGpuData> {
	private static final int WRITES_PER_FRAME = 2;

	private final String label;
	private final int size;
	private @Nullable DynamicGpuDataStorageMapped<T> storage;

	public UniformStream(String label, int size) {
		this.label = label;
		this.size = size;
	}

	public GpuBufferSlice write(T data) {
		DynamicGpuDataStorageMapped<T> storage = this.storage;
		if (storage == null) {
			storage = new DynamicGpuDataStorageMapped<>(this.label, this.size, GpuBuffer.USAGE_UNIFORM, WRITES_PER_FRAME);
			this.storage = storage;
		}
		return storage.writeData(data);
	}

	public void endFrame() {
		if (this.storage != null) {
			this.storage.endFrame();
		}
	}

	public void close() {
		if (this.storage != null) {
			this.storage.close();
			this.storage = null;
		}
	}
}
