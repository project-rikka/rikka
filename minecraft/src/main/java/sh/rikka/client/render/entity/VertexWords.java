package sh.rikka.client.render.entity;

import java.nio.ByteOrder;

public final class VertexWords {
	static {
		if (ByteOrder.nativeOrder() != ByteOrder.LITTLE_ENDIAN) {
			throw new IllegalStateException("direct vertex writing requires a little-endian platform");
		}
	}

	private VertexWords() {
	}

	public static long pair(int low, int high) {
		return (long) high << 32 | low & 0xFFFFFFFFL;
	}

	public static long pair(float low, float high) {
		return pair(Float.floatToRawIntBits(low), Float.floatToRawIntBits(high));
	}

	public static long pair(float low, int high) {
		return pair(Float.floatToRawIntBits(low), high);
	}
}
