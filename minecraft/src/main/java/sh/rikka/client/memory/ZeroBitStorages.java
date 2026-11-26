package sh.rikka.client.memory;

import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.util.ZeroBitStorage;

public final class ZeroBitStorages {
	private static final ConcurrentHashMap<Integer, ZeroBitStorage> BY_SIZE = new ConcurrentHashMap<>();

	private ZeroBitStorages() {
	}

	public static ZeroBitStorage ofSize(int size) {
		return BY_SIZE.computeIfAbsent(size, ZeroBitStorage::new);
	}
}
