package sh.rikka.client.render.terrain;

import com.mojang.renderpearl.api.GpuFormat;
import com.mojang.renderpearl.api.vertex.VertexFormat;
import com.mojang.renderpearl.api.vertex.VertexFormatElement;
import java.nio.ByteBuffer;
import java.util.Objects;
import org.lwjgl.system.MemoryUtil;

public final class CompactVertices {
	public static final int STRIDE = 16;
	public static final VertexFormat FORMAT = VertexFormat.builder(0)
		.addAttribute("PositionLight", GpuFormat.RGBA16_UINT)
		.addAttribute("Color", GpuFormat.RGBA8_UNORM)
		.addAttribute("UV0", GpuFormat.RG16_UINT)
		.build();

	private static final float POSITION_ORIGIN = 8.0F;
	private static final float POSITION_STEPS = 2048.0F;
	private static final float UV_STEPS = 32768.0F;
	private static final int UNSIGNED_SHORT_MAX = 0xFFFF;
	private static final int LIGHT_MAX = 0xFF;
	private static final int LIGHT_SHIFT = 8;
	private static final int COLOR_OFFSET = 8;
	private static final int UV_OFFSET = 12;

	private CompactVertices() {
	}

	public static int alignment(int alignment) {
		int common = alignment;
		for (int other = STRIDE; other != 0; ) {
			int remainder = common % other;
			common = other;
			other = remainder;
		}
		return alignment / common * STRIDE;
	}

	public static int compact(ByteBuffer vertices, VertexFormat format, int vertexCount) {
		int source = format.getVertexSize();
		int position = offset(format, "Position", GpuFormat.RGB32_FLOAT);
		int color = offset(format, "Color", GpuFormat.RGBA8_UNORM);
		int uv = offset(format, "UV0", GpuFormat.RG32_FLOAT);
		int light = offset(format, "UV2", GpuFormat.RG16_SINT);
		long base = MemoryUtil.memAddress(vertices);
		for (int vertex = 0; vertex < vertexCount; vertex++) {
			long from = base + (long) vertex * source;
			long to = base + (long) vertex * STRIDE;
			int x = quantize(MemoryUtil.memGetFloat(from + position) + POSITION_ORIGIN, POSITION_STEPS);
			int y = quantize(MemoryUtil.memGetFloat(from + position + 4) + POSITION_ORIGIN, POSITION_STEPS);
			int z = quantize(MemoryUtil.memGetFloat(from + position + 8) + POSITION_ORIGIN, POSITION_STEPS);
			int lightCoords = Math.min(MemoryUtil.memGetShort(from + light), LIGHT_MAX) | Math.min(MemoryUtil.memGetShort(from + light + 2), LIGHT_MAX) << LIGHT_SHIFT;
			int rgba = MemoryUtil.memGetInt(from + color);
			int u = quantize(MemoryUtil.memGetFloat(from + uv), UV_STEPS);
			int v = quantize(MemoryUtil.memGetFloat(from + uv + 4), UV_STEPS);
			MemoryUtil.memPutShort(to, (short) x);
			MemoryUtil.memPutShort(to + 2, (short) y);
			MemoryUtil.memPutShort(to + 4, (short) z);
			MemoryUtil.memPutShort(to + 6, (short) lightCoords);
			MemoryUtil.memPutInt(to + COLOR_OFFSET, rgba);
			MemoryUtil.memPutShort(to + UV_OFFSET, (short) u);
			MemoryUtil.memPutShort(to + UV_OFFSET + 2, (short) v);
		}
		return vertexCount * STRIDE;
	}

	private static int offset(VertexFormat format, String name, GpuFormat expected) {
		VertexFormatElement element = Objects.requireNonNull(format.getElement(name), name);
		if (element.format() != expected) {
			throw new IllegalStateException("terrain vertex element " + name + " is " + element.format() + ", not " + expected);
		}
		return element.offset();
	}

	private static int quantize(float value, float steps) {
		int quantized = Math.round(value * steps);
		if (quantized < 0 || quantized > UNSIGNED_SHORT_MAX) {
			throw new IllegalStateException("terrain vertex value " + value + " is outside the compact range");
		}
		return quantized;
	}
}
