package sh.rikka.client.memory;

import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;
import net.minecraft.util.ThreadingDetector;
import net.minecraft.world.level.chunk.PalettedContainer;

public final class PalettedContainerAccess {
	private static final String NAME = "PalettedContainer";
	public static final ThreadingDetector FREE = new ThreadingDetector(NAME);
	private static final ThreadingDetector HELD = new ThreadingDetector(NAME);
	private static final VarHandle DETECTOR = detectorHandle();

	private PalettedContainerAccess() {
	}

	public static void acquire(PalettedContainer<?> container) {
		while (true) {
			ThreadingDetector state = (ThreadingDetector) DETECTOR.getVolatile(container);
			if (state == FREE) {
				if (DETECTOR.compareAndSet(container, FREE, HELD)) {
					return;
				}
			} else if (state == HELD) {
				ThreadingDetector contended = new ThreadingDetector(NAME);
				contended.checkAndLock();
				if (DETECTOR.compareAndSet(container, HELD, contended)) {
					contended.checkAndLock();
					return;
				}
			} else {
				state.checkAndLock();
				return;
			}
		}
	}

	public static void release(PalettedContainer<?> container) {
		while (true) {
			ThreadingDetector state = (ThreadingDetector) DETECTOR.getVolatile(container);
			if (state == HELD) {
				if (DETECTOR.compareAndSet(container, HELD, FREE)) {
					return;
				}
			} else if (state == FREE) {
				ThreadingDetector unbalanced = new ThreadingDetector(NAME);
				if (DETECTOR.compareAndSet(container, FREE, unbalanced)) {
					unbalanced.checkAndUnlock();
					return;
				}
			} else {
				state.checkAndUnlock();
				return;
			}
		}
	}

	private static VarHandle detectorHandle() {
		try {
			return MethodHandles.privateLookupIn(PalettedContainer.class, MethodHandles.lookup())
				.findVarHandle(PalettedContainer.class, "threadingDetector", ThreadingDetector.class);
		} catch (ReflectiveOperationException exception) {
			throw new IllegalStateException("failed to access PalettedContainer.threadingDetector", exception);
		}
	}
}
