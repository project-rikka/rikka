package sh.rikka.client.simulation.thread;

import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.util.Optional;
import net.minecraft.util.Util;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class WorkerPriority {
	private static final Logger LOGGER = LoggerFactory.getLogger(WorkerPriority.class);
	private static final int NICENESS_STEP = 5;
	private static final Optional<Niceness> NICENESS = Util.getPlatform() == Util.OS.LINUX ? Niceness.link() : Optional.empty();
	private static final ThreadLocal<Boolean> LOWERED = ThreadLocal.withInitial(() -> Boolean.FALSE);

	private WorkerPriority() {
	}

	public static void lowerCurrentThread() {
		if (NICENESS.isEmpty() || LOWERED.get()) {
			return;
		}
		LOWERED.set(Boolean.TRUE);
		if (!NICENESS.get().raiseCurrentThread(NICENESS_STEP)) {
			LOGGER.warn("failed to lower the priority of {}", Thread.currentThread().getName());
		}
	}

	private record Niceness(MethodHandle getPriority, MethodHandle setPriority) {
		private static final int PROCESS_PRIORITY = 0;
		private static final int CALLING_THREAD = 0;
		private static final int MAXIMUM = 19;

		@SuppressWarnings("restricted")
		static Optional<Niceness> link() {
			Linker linker = Linker.nativeLinker();
			Optional<MemorySegment> getPriority = linker.defaultLookup().find("getpriority");
			Optional<MemorySegment> setPriority = linker.defaultLookup().find("setpriority");
			if (getPriority.isEmpty() || setPriority.isEmpty()) {
				LOGGER.warn("the C library has no thread priority functions");
				return Optional.empty();
			}
			return Optional.of(
				new Niceness(
					linker.downcallHandle(getPriority.get(), FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_INT, ValueLayout.JAVA_INT)),
					linker.downcallHandle(
						setPriority.get(),
						FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_INT, ValueLayout.JAVA_INT, ValueLayout.JAVA_INT)
					)
				)
			);
		}

		boolean raiseCurrentThread(int step) {
			try {
				int current = (int) this.getPriority.invokeExact(PROCESS_PRIORITY, CALLING_THREAD);
				return (int) this.setPriority.invokeExact(PROCESS_PRIORITY, CALLING_THREAD, Math.min(current + step, MAXIMUM)) == 0;
			} catch (Throwable failure) {
				throw new IllegalStateException("failed to change the thread priority", failure);
			}
		}
	}
}
