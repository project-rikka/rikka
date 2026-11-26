package sh.rikka.bench;

import jdk.jfr.Category;
import jdk.jfr.Event;
import jdk.jfr.Label;
import jdk.jfr.Name;
import jdk.jfr.StackTrace;

@Name("sh.rikka.bench.Frame")
@Label("Bench Frame")
@Category("Rikka Bench")
@StackTrace(false)
final class FrameEvent extends Event {
	@Label("Flight")
	String flight;

	@Label("Ticks")
	int ticks;

	@Label("CPU Nanoseconds")
	long cpuNanos;

	@Label("Allocated Bytes")
	long allocatedBytes;

	@Label("Acquire Nanoseconds")
	long acquireNanos;

	@Label("Submit Nanoseconds")
	long submitNanos;

	@Label("Present Nanoseconds")
	long presentNanos;

	@Label("Run Queue Nanoseconds")
	long runQueueNanos;
}
