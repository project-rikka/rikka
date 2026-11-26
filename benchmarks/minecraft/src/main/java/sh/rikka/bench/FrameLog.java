package sh.rikka.bench;

import com.google.gson.JsonObject;
import java.lang.management.GarbageCollectorMXBean;
import java.lang.management.ManagementFactory;
import java.util.Arrays;
import java.util.EnumMap;
import java.util.Map;
import org.jspecify.annotations.Nullable;

final class FrameLog {
	private static final int INITIAL_CAPACITY = 1 << 16;
	private static final double NANOS_PER_MILLI = 1.0e6;
	private static final double NANOS_PER_SECOND = 1.0e9;
	private static final double STALL_MEDIANS = 2.0;

	private final com.sun.management.ThreadMXBean threads = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
	private final Samples frames = new Samples();
	private final Samples tickFrames = new Samples();
	private final Samples ticks = new Samples();
	private final EnumMap<GpuSpan, Samples> gpu = new EnumMap<>(GpuSpan.class);
	private final EnumMap<Wait, Samples> waits = new EnumMap<>(Wait.class);
	private final long[] waitsInFrame = new long[Wait.values().length];
	private boolean recording;
	private String flight = "";
	private FrameEvent event = new FrameEvent();
	private long last;
	private long lastCpu;
	private long lastAllocated;
	private long tickStarted;
	private int ticksInFrame;
	private long allocatedAtStart;
	private long cpuAtStart;
	private long collectionsAtStart;
	private long collectionMillisAtStart;
	private long runQueueAtStart;
	private long lastRunQueue;
	private @Nullable RunQueueClock runQueue;

	private RunQueueClock runQueue() {
		RunQueueClock clock = this.runQueue;
		if (clock == null) {
			clock = new RunQueueClock();
			this.runQueue = clock;
		}
		return clock;
	}

	void start(String flight) {
		this.waits.clear();
		Arrays.fill(this.waitsInFrame, 0L);
		this.frames.clear();
		this.tickFrames.clear();
		this.ticks.clear();
		this.gpu.clear();
		this.recording = true;
		this.flight = flight;
		this.ticksInFrame = 0;
		this.last = System.nanoTime();
		this.allocatedAtStart = this.threads.getCurrentThreadAllocatedBytes();
		this.cpuAtStart = this.threads.getCurrentThreadCpuTime();
		this.lastAllocated = this.allocatedAtStart;
		this.lastCpu = this.cpuAtStart;
		this.collectionsAtStart = collections();
		this.collectionMillisAtStart = collectionMillis();
		this.runQueueAtStart = this.runQueue().waitedNanos();
		this.lastRunQueue = this.runQueueAtStart;
		this.event = new FrameEvent();
		this.event.begin();
	}

	void tickStarted() {
		this.tickStarted = System.nanoTime();
	}

	void tickFinished() {
		if (this.recording) {
			this.ticks.add(System.nanoTime() - this.tickStarted);
			this.ticksInFrame++;
		}
	}

	void waited(Wait wait, long nanos) {
		if (this.recording) {
			this.waits.computeIfAbsent(wait, key -> new Samples()).add(nanos);
			this.waitsInFrame[wait.ordinal()] += nanos;
		}
	}

	void frame() {
		if (!this.recording) {
			return;
		}
		long now = System.nanoTime();
		long duration = now - this.last;
		this.frames.add(duration);
		if (this.ticksInFrame > 0) {
			this.tickFrames.add(duration);
		}
		this.last = now;
		FrameEvent finished = this.event;
		finished.end();
		if (finished.shouldCommit()) {
			long cpu = this.threads.getCurrentThreadCpuTime();
			long allocated = this.threads.getCurrentThreadAllocatedBytes();
			finished.flight = this.flight;
			finished.ticks = this.ticksInFrame;
			finished.cpuNanos = cpu - this.lastCpu;
			finished.allocatedBytes = allocated - this.lastAllocated;
			finished.acquireNanos = this.waitsInFrame[Wait.ACQUIRE.ordinal()];
			finished.submitNanos = this.waitsInFrame[Wait.SUBMIT.ordinal()];
			finished.presentNanos = this.waitsInFrame[Wait.PRESENT.ordinal()];
			long waited = this.runQueue().waitedNanos();
			finished.runQueueNanos = waited - this.lastRunQueue;
			finished.commit();
			this.lastCpu = cpu;
			this.lastAllocated = allocated;
			this.lastRunQueue = waited;
		}
		Arrays.fill(this.waitsInFrame, 0L);
		this.ticksInFrame = 0;
		this.event = new FrameEvent();
		this.event.begin();
	}

	void gpuSpan(GpuSpan span, long nanos) {
		if (this.recording) {
			this.gpu.computeIfAbsent(span, key -> new Samples()).add(nanos);
		}
	}

	JsonObject stop() {
		this.recording = false;
		long[] sorted = this.frames.sorted();
		long total = Arrays.stream(sorted).sum();
		double median = percentile(sorted, 0.5);
		JsonObject result = new JsonObject();
		result.addProperty("frames", sorted.length);
		result.addProperty("seconds", total / NANOS_PER_SECOND);
		result.addProperty("fps_average", sorted.length * NANOS_PER_SECOND / total);
		result.addProperty("fps_low_1_percent", NANOS_PER_SECOND / slowestMean(sorted, 0.01));
		result.addProperty("fps_low_0_1_percent", NANOS_PER_SECOND / slowestMean(sorted, 0.001));
		result.addProperty("frame_ms_median", median / NANOS_PER_MILLI);
		result.addProperty("frame_ms_p90", percentile(sorted, 0.9) / NANOS_PER_MILLI);
		result.addProperty("frame_ms_p99", percentile(sorted, 0.99) / NANOS_PER_MILLI);
		result.addProperty("frame_ms_p99_9", percentile(sorted, 0.999) / NANOS_PER_MILLI);
		result.addProperty("frame_ms_max", sorted[sorted.length - 1] / NANOS_PER_MILLI);
		result.addProperty("stall_ms_per_second", stall(sorted, median * STALL_MEDIANS) / NANOS_PER_MILLI / (total / NANOS_PER_SECOND));
		result.addProperty("tick_frame_ms_median", medianMillis(this.tickFrames));
		result.addProperty("tick_ms_median", medianMillis(this.ticks));
		result.addProperty("tick_ms_p99", percentileMillis(this.ticks, 0.99));
		result.addProperty("cpu_ms_average", (this.threads.getCurrentThreadCpuTime() - this.cpuAtStart) / NANOS_PER_MILLI / sorted.length);
		for (Map.Entry<Wait, Samples> wait : this.waits.entrySet()) {
			long[] waitSorted = wait.getValue().sorted();
			result.addProperty("wait_ms_median_" + wait.getKey().label(), percentile(waitSorted, 0.5) / NANOS_PER_MILLI);
			result.addProperty("wait_ms_p99_" + wait.getKey().label(), percentile(waitSorted, 0.99) / NANOS_PER_MILLI);
			result.addProperty("wait_ms_per_second_" + wait.getKey().label(), Arrays.stream(waitSorted).sum() / NANOS_PER_MILLI / (total / NANOS_PER_SECOND));
		}
		for (Map.Entry<GpuSpan, Samples> span : this.gpu.entrySet()) {
			result.addProperty("gpu_ms_" + span.getKey().label(), medianMillis(span.getValue()));
		}
		result.addProperty("allocated_bytes_per_frame", (this.threads.getCurrentThreadAllocatedBytes() - this.allocatedAtStart) / (double) sorted.length);
		result.addProperty("collections", collections() - this.collectionsAtStart);
		result.addProperty("collection_ms", collectionMillis() - this.collectionMillisAtStart);
		result.addProperty("run_queue_ms_per_second", (this.runQueue().waitedNanos() - this.runQueueAtStart) / NANOS_PER_MILLI / (total / NANOS_PER_SECOND));
		return result;
	}

	private static double medianMillis(Samples samples) {
		return percentileMillis(samples, 0.5);
	}

	private static double percentileMillis(Samples samples, double fraction) {
		long[] sorted = samples.sorted();
		return sorted.length == 0 ? Double.NaN : percentile(sorted, fraction) / NANOS_PER_MILLI;
	}

	private static double percentile(long[] sorted, double fraction) {
		return sorted[Math.min(sorted.length - 1, (int) (sorted.length * fraction))];
	}

	private static double slowestMean(long[] sorted, double fraction) {
		int slowest = Math.max(1, (int) (sorted.length * fraction));
		return Arrays.stream(sorted, sorted.length - slowest, sorted.length).average().orElseThrow();
	}

	private static double stall(long[] sorted, double threshold) {
		double stalled = 0.0;
		for (int index = sorted.length - 1; index >= 0 && sorted[index] > threshold; index--) {
			stalled += sorted[index] - threshold;
		}
		return stalled;
	}

	private static long collections() {
		return ManagementFactory.getGarbageCollectorMXBeans().stream().mapToLong(GarbageCollectorMXBean::getCollectionCount).sum();
	}

	private static long collectionMillis() {
		return ManagementFactory.getGarbageCollectorMXBeans().stream().mapToLong(GarbageCollectorMXBean::getCollectionTime).sum();
	}

	private static final class Samples {
		private long[] values = new long[INITIAL_CAPACITY];
		private int count;

		void clear() {
			this.count = 0;
		}

		void add(long value) {
			if (this.count == this.values.length) {
				this.values = Arrays.copyOf(this.values, this.count * 2);
			}
			this.values[this.count++] = value;
		}

		long[] sorted() {
			long[] sorted = Arrays.copyOf(this.values, this.count);
			Arrays.sort(sorted);
			return sorted;
		}
	}
}
