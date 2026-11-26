package sh.rikka.client.render.terrain;

import com.mojang.renderpearl.api.vertex.VertexFormat;
import com.mojang.renderpearl.api.vertex.VertexFormatElement;
import java.nio.ByteBuffer;
import java.util.Objects;
import org.lwjgl.system.MemoryUtil;

public final class QuadBuckets {
	private static final int AXES = 3;
	private static final int VERTICES_PER_QUAD = 4;
	private static final float LOWEST_PLANE = -1.0F;
	private static final float HIGHEST_PLANE = 17.0F;
	private static final ThreadLocal<Scratch> SCRATCH = ThreadLocal.withInitial(Scratch::new);

	private QuadBuckets() {
	}

	public static int[] sort(ByteBuffer vertices, VertexFormat format, int vertexCount) {
		VertexFormatElement position = Objects.requireNonNull(format.getElement("Position"));
		int vertexBytes = format.getVertexSize();
		long quadBytes = (long) vertexBytes * VERTICES_PER_QUAD;
		int quads = vertexCount / VERTICES_PER_QUAD;
		long base = MemoryUtil.memAddress(vertices);
		Scratch scratch = SCRATCH.get();
		byte[] buckets = scratch.buckets(quads);
		int[] ends = new int[SectionTable.BUCKETS];
		boolean ordered = true;
		int highest = 0;
		for (int quad = 0; quad < quads; quad++) {
			int bucket = classify(base + quad * quadBytes + position.offset(), vertexBytes);
			buckets[quad] = (byte) bucket;
			ends[bucket]++;
			ordered &= bucket >= highest;
			highest = Math.max(highest, bucket);
		}
		int[] next = new int[SectionTable.BUCKETS];
		for (int bucket = 0, first = 0; bucket < SectionTable.BUCKETS; bucket++) {
			next[bucket] = first;
			first += ends[bucket];
			ends[bucket] = first;
		}
		if (!ordered) {
			long copy = scratch.memory(quads * quadBytes);
			for (int quad = 0; quad < quads; quad++) {
				MemoryUtil.memCopy(base + quad * quadBytes, copy + next[buckets[quad]]++ * quadBytes, quadBytes);
			}
			MemoryUtil.memCopy(copy, base, quads * quadBytes);
		}
		return ends;
	}

	private static int classify(long first, int vertexBytes) {
		long second = first + vertexBytes;
		long third = second + vertexBytes;
		long fourth = third + vertexBytes;
		int planar = -1;
		for (int axis = 0; axis < AXES; axis++) {
			long offset = (long) axis * Float.BYTES;
			float plane = MemoryUtil.memGetFloat(first + offset);
			if (plane == MemoryUtil.memGetFloat(second + offset) && plane == MemoryUtil.memGetFloat(third + offset) && plane == MemoryUtil.memGetFloat(fourth + offset)) {
				if (planar >= 0 || !(plane >= LOWEST_PLANE && plane <= HIGHEST_PLANE)) {
					return SectionTable.UNASSIGNED_BUCKET;
				}
				planar = axis;
			}
		}
		if (planar < 0) {
			return SectionTable.UNASSIGNED_BUCKET;
		}
		int u = (planar + 1) % AXES * Float.BYTES;
		int v = (planar + 2) % AXES * Float.BYTES;
		double firstU = MemoryUtil.memGetFloat(first + u);
		double firstV = MemoryUtil.memGetFloat(first + v);
		double thirdU = MemoryUtil.memGetFloat(third + u);
		double thirdV = MemoryUtil.memGetFloat(third + v);
		double front = area(firstU, firstV, MemoryUtil.memGetFloat(second + u), MemoryUtil.memGetFloat(second + v), thirdU, thirdV);
		double back = area(thirdU, thirdV, MemoryUtil.memGetFloat(fourth + u), MemoryUtil.memGetFloat(fourth + v), firstU, firstV);
		if (front > 0.0 && back > 0.0) {
			return planar;
		}
		if (front < 0.0 && back < 0.0) {
			return SectionTable.UNASSIGNED_BUCKET + 1 + planar;
		}
		return SectionTable.UNASSIGNED_BUCKET;
	}

	private static double area(double originU, double originV, double nextU, double nextV, double lastU, double lastV) {
		return (nextU - originU) * (lastV - originV) - (nextV - originV) * (lastU - originU);
	}

	private static final class Scratch {
		private byte[] buckets = new byte[0];
		private ByteBuffer memory = ByteBuffer.allocateDirect(0);

		byte[] buckets(int quads) {
			if (this.buckets.length < quads) {
				this.buckets = new byte[Integer.highestOneBit(quads) * 2];
			}
			return this.buckets;
		}

		long memory(long bytes) {
			if (this.memory.capacity() < bytes) {
				this.memory = ByteBuffer.allocateDirect(Math.toIntExact(Long.highestOneBit(bytes) * 2));
			}
			return MemoryUtil.memAddress(this.memory);
		}
	}
}
