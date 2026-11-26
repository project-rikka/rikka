package sh.rikka.client.render.entity;

import java.nio.BufferOverflowException;
import java.nio.ByteBuffer;
import java.util.Arrays;
import java.util.concurrent.Executor;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import org.lwjgl.system.MemoryUtil;

public final class VertexCopyBatch {
	private static final int HELPER_COUNT = 2;
	private static final long PARALLEL_BYTES = 1L << 20;
	private static final long CHUNK_BYTES = 1L << 17;
	private static final Executor HELPERS = Executors.newFixedThreadPool(HELPER_COUNT, runnable -> {
		Thread thread = new Thread(runnable, "Rikka Vertex Copy");
		thread.setDaemon(true);
		return thread;
	});

	private long[] sources = new long[256];
	private long[] targets = new long[256];
	private int[] sizes = new int[256];
	private int count;
	private long totalBytes;

	public void add(ByteBuffer target, ByteBuffer source) {
		int size = source.remaining();
		if (size > target.remaining()) {
			throw new BufferOverflowException();
		}
		if (this.count == this.sizes.length) {
			int capacity = this.count * 2;
			this.sources = Arrays.copyOf(this.sources, capacity);
			this.targets = Arrays.copyOf(this.targets, capacity);
			this.sizes = Arrays.copyOf(this.sizes, capacity);
		}
		this.sources[this.count] = MemoryUtil.memAddress(source);
		this.targets[this.count] = MemoryUtil.memAddress(target);
		this.sizes[this.count] = size;
		this.count++;
		this.totalBytes += size;
		target.position(target.position() + size);
		source.position(source.limit());
	}

	public void copy() {
		if (this.count == 0) {
			return;
		}
		Pass pass = new Pass(this.sources, this.targets, this.sizes, this.count, this.totalBytes);
		if (pass.chunkCount > 1) {
			for (int helper = 0; helper < HELPER_COUNT; helper++) {
				HELPERS.execute(pass::drain);
			}
		}
		pass.drain();
		pass.awaitFinished();
		this.count = 0;
		this.totalBytes = 0L;
	}

	private static final class Pass {
		private final long[] sources;
		private final long[] targets;
		private final int[] sizes;
		private final int count;
		private final long totalBytes;
		private final int chunkCount;
		private final AtomicInteger nextChunk = new AtomicInteger();
		private final AtomicInteger finishedChunks = new AtomicInteger();
		private final AtomicReference<Throwable> failure = new AtomicReference<>();

		private Pass(long[] sources, long[] targets, int[] sizes, int count, long totalBytes) {
			this.sources = sources;
			this.targets = targets;
			this.sizes = sizes;
			this.count = count;
			this.totalBytes = totalBytes;
			this.chunkCount = totalBytes < PARALLEL_BYTES ? 1 : (int) ((totalBytes + CHUNK_BYTES - 1) / CHUNK_BYTES);
		}

		private void drain() {
			int chunk;
			while ((chunk = this.nextChunk.getAndIncrement()) < this.chunkCount) {
				try {
					long from = chunk * CHUNK_BYTES;
					long to = this.chunkCount == 1 ? this.totalBytes : Math.min(this.totalBytes, from + CHUNK_BYTES);
					this.copyRange(from, to);
				} catch (Throwable throwable) {
					this.failure.compareAndSet(null, throwable);
				} finally {
					this.finishedChunks.incrementAndGet();
				}
			}
		}

		private void awaitFinished() {
			while (this.finishedChunks.get() < this.chunkCount) {
				Thread.onSpinWait();
			}
			Throwable throwable = this.failure.get();
			if (throwable != null) {
				throw new IllegalStateException("failed to copy vertices", throwable);
			}
		}

		private void copyRange(long from, long to) {
			long offset = 0L;
			for (int job = 0; job < this.count && offset < to; job++) {
				long end = offset + this.sizes[job];
				if (end > from) {
					long start = Math.max(from, offset);
					long stop = Math.min(to, end);
					MemoryUtil.memCopy(this.sources[job] + (start - offset), this.targets[job] + (start - offset), stop - start);
				}
				offset = end;
			}
		}
	}
}
