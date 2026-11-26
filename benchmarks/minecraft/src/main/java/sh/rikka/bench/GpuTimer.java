package sh.rikka.bench;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.commands.GpuQueryPool;
import com.mojang.renderpearl.api.commands.RenderPass;
import com.mojang.renderpearl.api.device.GpuDevice;
import java.util.OptionalLong;
import org.jspecify.annotations.Nullable;

final class GpuTimer {
	private static final int FRAMES = 64;
	private static final int LATENCY = 8;
	private static final int SPANS = GpuSpan.values().length;

	private final Sink sink;
	private @Nullable GpuQueryPool pool;
	private long frame;
	private int written;

	@FunctionalInterface
	interface Sink {
		void accept(GpuSpan span, long nanos);
	}

	GpuTimer(Sink sink) {
		this.sink = sink;
	}

	void begin(GpuSpan span, @Nullable RenderPass pass) {
		this.stamp(span, 0, pass);
	}

	void end(GpuSpan span, @Nullable RenderPass pass) {
		this.stamp(span, 1, pass);
	}

	void finishFrame() {
		GpuQueryPool queries = this.pool;
		if (queries == null) {
			return;
		}
		int complete = this.written;
		this.written = 0;
		this.frame++;
		if (this.frame <= LATENCY) {
			return;
		}
		long read = this.frame - LATENCY;
		double period = RenderSystem.getDevice().getDeviceInfo().timestampPeriod();
		OptionalLong[] stamps = queries.getValues(query(read, GpuSpan.FRAME, 0), SPANS * 2);
		for (GpuSpan span : GpuSpan.values()) {
			OptionalLong start = stamps[span.ordinal() * 2];
			OptionalLong stop = stamps[span.ordinal() * 2 + 1];
			if ((complete >> span.ordinal() * 2 & 3) == 3 && start.isPresent() && stop.isPresent()) {
				this.sink.accept(span, (long) ((stop.getAsLong() - start.getAsLong()) * period));
			}
		}
	}

	private void stamp(GpuSpan span, int edge, @Nullable RenderPass pass) {
		GpuDevice device = RenderSystem.getDevice();
		if (this.pool == null) {
			this.pool = device.createTimestampQueryPool(FRAMES * SPANS * 2);
		}
		int query = query(this.frame, span, edge);
		if (pass != null) {
			pass.writeTimestamp(this.pool, query);
		} else {
			device.createCommandEncoder().writeTimestamp(this.pool, query);
		}
		this.written |= 1 << span.ordinal() * 2 + edge;
	}

	private static int query(long frame, GpuSpan span, int edge) {
		return ((int) (frame % FRAMES) * SPANS + span.ordinal()) * 2 + edge;
	}
}
