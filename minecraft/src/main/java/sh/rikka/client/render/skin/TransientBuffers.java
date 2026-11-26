package sh.rikka.client.render.skin;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import com.mojang.renderpearl.api.commands.GpuFence;
import com.mojang.renderpearl.api.device.GpuDevice;
import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;
import org.lwjgl.system.MemoryUtil;

final class TransientBuffers implements AutoCloseable {
	private static final long GRANULARITY = 1L << 16;
	private static final long REUSE_FACTOR = 4L;

	private final String label;
	private final @GpuBuffer.Usage int usage;
	private final List<Mapped> available = new ArrayList<>();
	private final List<Mapped> used = new ArrayList<>();
	private final List<Retired> retired = new ArrayList<>();

	TransientBuffers(String label, @GpuBuffer.Usage int usage) {
		this.label = label;
		this.usage = usage | GpuBuffer.USAGE_MAP_WRITE;
	}

	Mapped acquire(long bytes) {
		long size = Math.ceilDiv(bytes, GRANULARITY) * GRANULARITY;
		Mapped buffer = takeAvailable(size);
		if (buffer == null) {
			buffer = Mapped.create(RenderSystem.getDevice().createBuffer(() -> this.label, this.usage, size));
		}
		this.used.add(buffer);
		return buffer;
	}

	void endFrame() {
		GpuDevice device = RenderSystem.getDevice();
		if (!this.used.isEmpty()) {
			this.retired.add(new Retired(List.copyOf(this.used), device.createCommandEncoder().createFence()));
			this.used.clear();
		}
		this.available.forEach(Mapped::close);
		this.available.clear();
		this.retired.removeIf(retired -> {
			if (!retired.fence().awaitCompletion(0L)) {
				return false;
			}
			retired.fence().close();
			this.available.addAll(retired.buffers());
			return true;
		});
	}

	private @Nullable Mapped takeAvailable(long size) {
		int best = -1;
		long bestSize = size * REUSE_FACTOR + 1L;
		for (int index = 0; index < this.available.size(); index++) {
			long candidate = this.available.get(index).buffer().size();
			if (candidate >= size && candidate < bestSize) {
				best = index;
				bestSize = candidate;
			}
		}
		return best < 0 ? null : this.available.remove(best);
	}

	@Override
	public void close() {
		this.available.forEach(Mapped::close);
		this.used.forEach(Mapped::close);
		for (Retired retired : this.retired) {
			retired.buffers().forEach(Mapped::close);
			retired.fence().close();
		}
		this.available.clear();
		this.used.clear();
		this.retired.clear();
	}

	record Mapped(GpuBuffer buffer, GpuBufferSlice.MappedView view, long address) implements AutoCloseable {
		private static Mapped create(GpuBuffer buffer) {
			GpuBufferSlice.MappedView view = buffer.map(false, true);
			return new Mapped(buffer, view, MemoryUtil.memAddress(view.data()));
		}

		@Override
		public void close() {
			this.view.close();
			this.buffer.close();
		}
	}

	private record Retired(List<Mapped> buffers, GpuFence fence) {
	}
}
