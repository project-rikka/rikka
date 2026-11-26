package sh.rikka.bench;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;

final class RunQueueClock {
	private static final Path SCHEDULER_STATISTICS = Path.of("/proc/thread-self/schedstat");
	private static final int WAITED_FIELD = 1;

	private final FileChannel channel;
	private final ByteBuffer buffer = ByteBuffer.allocateDirect(128);

	RunQueueClock() {
		try {
			this.channel = FileChannel.open(SCHEDULER_STATISTICS, StandardOpenOption.READ);
		} catch (IOException error) {
			throw new UncheckedIOException("failed to open " + SCHEDULER_STATISTICS, error);
		}
	}

	long waitedNanos() {
		this.buffer.clear();
		try {
			this.channel.read(this.buffer, 0L);
		} catch (IOException error) {
			throw new UncheckedIOException("failed to read " + SCHEDULER_STATISTICS, error);
		}
		this.buffer.flip();
		long value = 0L;
		int field = 0;
		while (this.buffer.hasRemaining()) {
			byte character = this.buffer.get();
			if (character == ' ') {
				if (field == WAITED_FIELD) {
					return value;
				}
				field++;
				value = 0L;
			} else {
				value = value * 10L + (character - '0');
			}
		}
		throw new IllegalStateException("unexpected scheduler statistics");
	}
}
